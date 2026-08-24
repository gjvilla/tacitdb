//! A strict, line-based parser for the decision-record corpus format.
//!
//! Deliberately not a YAML dependency. Three reasons specific to this file:
//! one of its blocks is not valid YAML (`source: phase interview / U-2 verdict
//! round (trigger fired: data model ...)` — a bare `:` inside a plain scalar),
//! so a conforming parser rejects the record and the only fixes are editing
//! the founding record to suit a dependency or pre-cleaning the text; the
//! inline `#` comments carry meaning here (two of them qualify a `state:`
//! line) and every conforming parser drops them; and the format is one we
//! own, so a first-`:` split reads it correctly and touches nothing.
//!
//! Strictness is the point: unknown keys, unlabelled sections, and
//! unresolvable evidence are hard errors. A corpus about honesty must not
//! silently drop what it does not understand.

use std::collections::BTreeMap;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ParseError {
    #[error("record {record}: unknown yaml key {key:?}")]
    UnknownKey { record: String, key: String },

    #[error("record {record}: duplicate yaml key {key:?}")]
    DuplicateKey { record: String, key: String },

    #[error("record {record}: missing required yaml key {key:?}")]
    MissingKey { record: String, key: String },

    #[error("record {record}: yaml id {found:?} does not match heading id {expected:?}")]
    IdMismatch { record: String, expected: String, found: String },

    #[error("record {record}: no yaml block")]
    MissingYaml { record: String },

    #[error("record {record}: prose before the yaml block would be dropped: {line:?}")]
    ProseBeforeYaml { record: String, line: String },

    #[error("record {record}: no Assertion section")]
    MissingAssertion { record: String },

    #[error("record {record}: continuation line before any key: {line:?}")]
    OrphanContinuation { record: String, line: String },

    #[error("record {record}: prose outside any labelled section: {text:?}")]
    UnlabelledProse { record: String, text: String },

    #[error("record {record}: duplicate section label {label:?}")]
    DuplicateSection { record: String, label: String },

    #[error("heading is not a record heading: {0:?}")]
    BadHeading(String),

    #[error("register row has {cells} cells, expected 4: {row:?}")]
    BadRegisterRow { row: String, cells: usize },

    #[error("register lists {id} twice")]
    DuplicateUnknown { id: String },
}

/// Keys the ingest understands, partitioned by role. Anything else is an error.
pub const ENVELOPE_KEYS: [&str; 6] =
    ["author", "recorded", "valid_from", "source", "evidence", "review_trigger"];
pub const CONTENT_KEYS: [&str; 1] = ["score_by"];
/// `id` and `state` are instructions to the ingester, never stored: `state` in
/// particular is a control field that selects which verdict to transcribe.
pub const CONTROL_KEYS: [&str; 2] = ["id", "state"];

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedRecord {
    /// `D-0001`, `H-0001`.
    pub id: String,
    /// The heading text after the separator.
    pub title: String,
    pub yaml: BTreeMap<String, String>,
    /// Any inline `#` comment stripped from a yaml line, kept because two of
    /// them qualify their `state:` value.
    pub comments: BTreeMap<String, String>,
    /// Labelled prose sections, in source order.
    pub sections: Vec<(String, String)>,
    /// The record's full text, for reference scanning.
    pub raw: String,
    /// Where the record sits in its document: 1-based, inclusive, heading line
    /// through last body line. Carried so the keeper can ask git who wrote it
    /// before transcribing a person's verdict from it (D-0025).
    pub lines: (usize, usize),
}

impl ParsedRecord {
    pub fn section(&self, label: &str) -> Option<&str> {
        self.sections.iter().find(|(l, _)| l == label).map(|(_, body)| body.as_str())
    }

    pub fn require(&self, key: &str) -> Result<&str, ParseError> {
        self.yaml
            .get(key)
            .map(String::as_str)
            .ok_or_else(|| ParseError::MissingKey { record: self.id.clone(), key: key.into() })
    }
}

/// Split the document into record blocks and parse each.
pub fn parse_corpus(text: &str) -> Result<Vec<ParsedRecord>, ParseError> {
    let mut records = Vec::new();
    let mut current: Option<(String, String, Vec<&str>, usize)> = None;
    let mut seen = 0usize;

    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        seen = number;
        if let Some(heading) = line.strip_prefix("## ") {
            if let Some((id, title, body, start)) = current.take() {
                records.push(parse_record(id, title, &body.join("\n"), (start, number - 1))?);
            }
            let (id, title) = split_heading(heading)?;
            current = Some((id, title, Vec::new(), number));
        } else if let Some((_, _, body, _)) = current.as_mut() {
            body.push(line);
        }
    }
    if let Some((id, title, body, start)) = current.take() {
        records.push(parse_record(id, title, &body.join("\n"), (start, seen))?);
    }
    Ok(records)
}

/// `D-0001 · The forces driving the build`
fn split_heading(heading: &str) -> Result<(String, String), ParseError> {
    let (id, title) = heading
        .split_once('·')
        .ok_or_else(|| ParseError::BadHeading(heading.to_string()))?;
    let id = id.trim().to_string();
    if !is_record_id(&id) {
        return Err(ParseError::BadHeading(heading.to_string()));
    }
    Ok((id, title.trim().to_string()))
}

/// `D-0001` / `H-0001`: one of `D`/`H`, a hyphen, then exactly four digits.
pub fn is_record_id(candidate: &str) -> bool {
    let bytes = candidate.as_bytes();
    bytes.len() == 6
        && (bytes[0] == b'D' || bytes[0] == b'H')
        && bytes[1] == b'-'
        && bytes[2..].iter().all(u8::is_ascii_digit)
}

/// `U-5` / `U-22`: the register numbers its unknowns without zero-padding, so
/// the digit count varies where the decision corpus fixes it at four.
pub fn is_unknown_id(candidate: &str) -> bool {
    let bytes = candidate.as_bytes();
    (3..=5).contains(&bytes.len())
        && bytes[0] == b'U'
        && bytes[1] == b'-'
        && bytes[2..].iter().all(u8::is_ascii_digit)
}

fn parse_record(
    id: String,
    title: String,
    body: &str,
    lines: (usize, usize),
) -> Result<ParsedRecord, ParseError> {
    let (yaml_block, prose) = split_yaml(&id, body)?;
    let YamlBlock { values: yaml, comments } = parse_yaml(&id, &yaml_block)?;

    if let Some(found) = yaml.get("id")
        && *found != id
    {
        return Err(ParseError::IdMismatch {
            record: id.clone(),
            expected: id.clone(),
            found: found.clone(),
        });
    }

    let sections = parse_sections(&id, &prose)?;
    // An empty claim body must never reach the ledger and collect a
    // transcribed promote verdict: a record with nothing to assert is a parse
    // failure, not a promotable claim.
    let has_substance = sections
        .iter()
        .any(|(label, body)| (label == "Assertion" || label == "Hypothesis") && !body.is_empty());
    if !has_substance {
        return Err(ParseError::MissingAssertion { record: id });
    }
    Ok(ParsedRecord { id, title, yaml, comments, sections, raw: body.to_string(), lines })
}

fn split_yaml(id: &str, body: &str) -> Result<(String, String), ParseError> {
    let mut yaml = Vec::new();
    let mut prose = Vec::new();
    let mut state = 0; // 0 = before fence, 1 = inside, 2 = after
    for line in body.lines() {
        match state {
            0 if line.trim_start().starts_with("```") => state = 1,
            // Text above the fence would otherwise vanish: it reaches neither
            // the yaml block nor the sections, so the record would ingest with
            // its assertion missing — and still collect a transcribed promote
            // verdict for the hollow remainder.
            0 if !line.trim().is_empty() => {
                return Err(ParseError::ProseBeforeYaml {
                    record: id.to_string(),
                    line: line.trim().chars().take(60).collect(),
                });
            }
            0 => {}
            1 if line.trim_start().starts_with("```") => state = 2,
            1 => yaml.push(line),
            _ => prose.push(line),
        }
    }
    if state != 2 {
        return Err(ParseError::MissingYaml { record: id.to_string() });
    }
    Ok((yaml.join("\n"), prose.join("\n")))
}

/// A parsed yaml block: values, plus the inline comments a conforming parser
/// would have dropped.
struct YamlBlock {
    values: BTreeMap<String, String>,
    comments: BTreeMap<String, String>,
}

/// Line-based: split each key line on its FIRST `:`; a line indented and not
/// key-shaped continues the previous value.
fn parse_yaml(id: &str, block: &str) -> Result<YamlBlock, ParseError> {
    let mut values: BTreeMap<String, String> = BTreeMap::new();
    let mut comments: BTreeMap<String, String> = BTreeMap::new();
    let mut last_key: Option<String> = None;

    for line in block.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let indented = line.starts_with(' ');
        let key_shaped = !indented
            && line
                .split_once(':')
                .is_some_and(|(k, _)| !k.is_empty() && k.chars().all(is_key_char));

        if key_shaped {
            let (key, rest) = line.split_once(':').expect("key-shaped");
            let key = key.trim().to_string();
            if !known_key(&key) {
                return Err(ParseError::UnknownKey { record: id.to_string(), key });
            }
            if values.contains_key(&key) {
                return Err(ParseError::DuplicateKey { record: id.to_string(), key });
            }
            let (value, comment) = strip_comment(rest.trim());
            if let Some(comment) = comment {
                comments.insert(key.clone(), comment);
            }
            values.insert(key.clone(), value);
            last_key = Some(key);
        } else {
            let Some(key) = last_key.clone() else {
                return Err(ParseError::OrphanContinuation {
                    record: id.to_string(),
                    line: line.to_string(),
                });
            };
            let (extra, comment) = strip_comment(line.trim());
            if let Some(comment) = comment {
                comments.entry(key.clone()).or_insert(comment);
            }
            let value = values.get_mut(&key).expect("key exists");
            value.push(' ');
            value.push_str(&extra);
        }
    }
    Ok(YamlBlock { values, comments })
}

fn is_key_char(c: char) -> bool {
    c.is_ascii_lowercase() || c == '_'
}

fn known_key(key: &str) -> bool {
    ENVELOPE_KEYS.contains(&key) || CONTENT_KEYS.contains(&key) || CONTROL_KEYS.contains(&key)
}

/// Strip a trailing ` # comment`. Only whitespace-preceded `#` counts, so a
/// `#` inside a value is left alone.
fn strip_comment(value: &str) -> (String, Option<String>) {
    match value.find(" #") {
        Some(ix) => (
            value[..ix].trim_end().to_string(),
            Some(value[ix + 2..].trim().to_string()),
        ),
        None => (value.to_string(), None),
    }
}

/// Labelled prose sections. A section opens on a **paragraph** that begins
/// with a bold run ending in a period — `**Assertion.**`. The trailing period
/// is load-bearing: `**required envelope**` appears mid-paragraph in D-0004,
/// and a rule that only checked for a leading `**` would split that assertion
/// into a bogus section while every count-based check still passed.
fn parse_sections(id: &str, prose: &str) -> Result<Vec<(String, String)>, ParseError> {
    let mut sections: Vec<(String, String)> = Vec::new();
    for paragraph in prose.split("\n\n") {
        let paragraph = paragraph.trim();
        if paragraph.is_empty() || paragraph == "---" {
            continue;
        }
        match section_label(paragraph) {
            Some((label, body)) => {
                if sections.iter().any(|(l, _)| *l == label) {
                    return Err(ParseError::DuplicateSection { record: id.to_string(), label });
                }
                sections.push((label, unwrap_lines(body)));
            }
            None => {
                let Some((_, body)) = sections.last_mut() else {
                    return Err(ParseError::UnlabelledProse {
                        record: id.to_string(),
                        text: paragraph.chars().take(60).collect(),
                    });
                };
                body.push(' ');
                body.push_str(&unwrap_lines(paragraph));
            }
        }
    }
    Ok(sections)
}

fn section_label(paragraph: &str) -> Option<(String, &str)> {
    let rest = paragraph.strip_prefix("**")?;
    let end = rest.find("**")?;
    let label = &rest[..end];
    let label = label.strip_suffix('.')?;
    if label.is_empty() || label.contains('\n') {
        return None;
    }
    Some((label.to_string(), rest[end + 2..].trim_start()))
}

/// Fold hard-wrapped lines into one line.
fn unwrap_lines(text: &str) -> String {
    text.split('\n').map(str::trim).collect::<Vec<_>>().join(" ").trim().to_string()
}

/// Every corpus id mentioned in `text` — decision ids (`D-0001`, `H-0001`) and
/// register ids (`U-5`) alike — excluding `self_id`, in first-mention order.
/// A bare textual mention is all this observes, hence the predicate name
/// `mentions` rather than anything stronger.
pub fn mentioned_ids(text: &str, self_id: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut found: Vec<String> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        let is_prefix = matches!(c, b'D' | b'H' | b'U');
        let boundary_ok =
            i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'-');
        if !is_prefix || !boundary_ok || bytes.get(i + 1) != Some(&b'-') {
            i += 1;
            continue;
        }
        // Take the maximal digit run, so "U-10" never reads as "U-1".
        let digits_start = i + 2;
        let mut end = digits_start;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        if end == digits_start || bytes.get(end).is_some_and(u8::is_ascii_alphanumeric) {
            i += 1;
            continue;
        }
        let candidate = &text[i..end];
        if is_record_id(candidate) || is_unknown_id(candidate) {
            let id = candidate.to_string();
            if id != self_id && !found.contains(&id) {
                found.push(id);
            }
            i = end;
        } else {
            i += 1;
        }
    }
    found
}

/// Split an evidence list body (`a, b §1, §2`) into entries, merging orphan
/// section fragments back into the entry they belong to — `[design/001.md
/// §1.1, §5]` is one file with two spans, not two files.
pub fn split_evidence(list: &str) -> Vec<String> {
    let inner = list.trim().trim_start_matches('[').trim_end_matches(']');
    let mut entries: Vec<String> = Vec::new();
    for part in inner.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if part.starts_with('§') && !entries.is_empty() {
            let last = entries.last_mut().expect("non-empty");
            last.push_str(", ");
            last.push_str(part);
        } else {
            entries.push(part.to_string());
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_record_with_continuations_and_comments() {
        let doc = "\
## D-0001 · The forces driving the build

```yaml
id: D-0001
state: promoted        # the decision to decide is promoted
author: Greg Villa
source: founding-interview / round 1
review_trigger: any force resolved externally — e.g. licensing clarified,
  or an incumbent ships engine-level provenance
```

**Assertion.** Four forces jointly motivate the build, and the wrapped
second line belongs to the same paragraph.

**Forces.** All four were selected; none alone would justify it.
";
        let records = parse_corpus(doc).unwrap();
        assert_eq!(records.len(), 1);
        let r = &records[0];
        assert_eq!(r.id, "D-0001");
        assert_eq!(r.title, "The forces driving the build");
        assert_eq!(r.yaml["state"], "promoted");
        assert_eq!(r.comments["state"], "the decision to decide is promoted");
        assert!(r.yaml["review_trigger"].contains("licensing clarified, or an incumbent"));
        assert_eq!(r.sections.len(), 2);
        assert!(r.section("Assertion").unwrap().ends_with("same paragraph."));
        assert!(r.section("Forces").is_some());
    }

    /// The D-0004 trap: a bold run mid-paragraph with no trailing period must
    /// not open a section.
    #[test]
    fn bold_runs_without_a_trailing_period_are_not_sections() {
        let doc = "\
## D-0004 · Unit of memory

```yaml
id: D-0004
state: promoted
```

**Assertion.** Tacit's atomic record is an **assertion** wrapped in a
**required envelope**: source, author, valid-time. Content stays flexible.

**Alternatives rejected.** Plain property graph.
";
        let records = parse_corpus(doc).unwrap();
        let r = &records[0];
        assert_eq!(
            r.sections.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>(),
            vec!["Assertion", "Alternatives rejected"]
        );
        assert!(r.section("Assertion").unwrap().contains("required envelope"));
    }

    #[test]
    fn unknown_keys_are_hard_errors() {
        let doc = "## D-0001 · T\n\n```yaml\nid: D-0001\nbogus: 1\n```\n\n**Assertion.** x.\n";
        assert!(matches!(parse_corpus(doc), Err(ParseError::UnknownKey { .. })));
    }

    #[test]
    fn unlabelled_prose_is_a_hard_error() {
        let doc = "## D-0001 · T\n\n```yaml\nid: D-0001\n```\n\nfloating prose.\n";
        assert!(matches!(parse_corpus(doc), Err(ParseError::UnlabelledProse { .. })));
    }

    #[test]
    fn evidence_merges_orphan_section_fragments() {
        assert_eq!(
            split_evidence("[design/001-data-model.md §1.1, §5]"),
            vec!["design/001-data-model.md §1.1, §5"]
        );
        assert_eq!(
            split_evidence("[REQUIREMENTS.md, REGISTER.md]"),
            vec!["REQUIREMENTS.md", "REGISTER.md"]
        );
    }

    #[test]
    fn mentions_finds_ids_but_not_the_record_itself() {
        let found = mentioned_ids("resolves U-1 per D-0012 and D-0012 again; see H-0001", "D-0006");
        assert_eq!(found, vec!["U-1", "D-0012", "H-0001"]);
        assert!(mentioned_ids("D-00123 is not an id", "D-0001").is_empty());
        assert!(mentioned_ids("R-5 and D-001 are not corpus ids", "D-0001").is_empty());
    }

    /// Register ids are not zero-padded, so the scan must take the maximal
    /// digit run or "U-10" reads as "U-1".
    #[test]
    fn variable_length_register_ids_are_read_whole() {
        assert_eq!(mentioned_ids("see U-10 and U-1", "U-5"), vec!["U-10", "U-1"]);
        assert_eq!(mentioned_ids("U-22 is the last", "U-5"), vec!["U-22"]);
        assert!(mentioned_ids("U-1 only", "U-1").is_empty(), "self is excluded");
    }
}

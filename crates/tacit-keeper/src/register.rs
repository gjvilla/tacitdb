//! Parse and ingest the four-rooms register's known unknowns as gap records.
//!
//! A registered gap is what makes honest abstention possible: it is the
//! difference between "the record has nothing" and "this is a named open
//! question, and here is its trigger". Room 2 of the register is exactly that
//! list, so it belongs in the ledger rather than only in a document.
//!
//! Resolved entries are not dropped — history is never rewritten. Each is
//! transcribed as an `Answer` verdict naming the promoted decision claim that
//! settled it, which the engine will refuse unless that claim really is
//! promoted.

use crate::parse::{ParseError, is_unknown_id, mentioned_ids};

/// One row of Room 2.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedUnknown {
    /// `U-5`.
    pub id: String,
    pub question: String,
    /// The event that forces the decision. `—` for resolved rows.
    pub trigger: String,
    pub notes: String,
    pub resolved: Option<Resolution>,
    /// The row exactly as written, so a re-ingest can tell whether it changed.
    pub raw: String,
    /// Which line of the register the row sits on, 1-based (D-0025).
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Resolution {
    /// `2026-08-23`, as written.
    pub date: String,
    /// The decision record that settled it, when the row names one.
    pub by: Option<String>,
}

impl ParsedUnknown {
    /// Every corpus id this row names, excluding itself.
    pub fn mentions(&self) -> Vec<String> {
        let text = format!("{} {} {}", self.question, self.trigger, self.notes);
        mentioned_ids(&text, &self.id)
    }
}

/// Parse Room 2's table. Rows elsewhere in the document are ignored; a row
/// that starts like an unknown but does not have four cells is a hard error,
/// because a silently skipped unknown is precisely the bluff the register
/// exists to prevent.
pub fn parse_register(text: &str) -> Result<Vec<ParsedUnknown>, ParseError> {
    let mut unknowns: Vec<ParsedUnknown> = Vec::new();

    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with("| U-") {
            continue;
        }
        let cells: Vec<&str> = trimmed
            .strip_prefix('|')
            .and_then(|l| l.strip_suffix('|'))
            .unwrap_or(trimmed)
            .split('|')
            .map(str::trim)
            .collect();
        if cells.len() != 4 {
            return Err(ParseError::BadRegisterRow {
                row: trimmed.chars().take(60).collect(),
                cells: cells.len(),
            });
        }

        let id = cells[0].to_string();
        if !is_unknown_id(&id) {
            return Err(ParseError::BadRegisterRow {
                row: trimmed.chars().take(60).collect(),
                cells: cells.len(),
            });
        }
        if unknowns.iter().any(|u| u.id == id) {
            return Err(ParseError::DuplicateUnknown { id });
        }

        let question = cells[1].to_string();
        let resolved = parse_resolution(&id, &question);
        unknowns.push(ParsedUnknown {
            id,
            question,
            trigger: cells[2].to_string(),
            notes: cells[3].to_string(),
            resolved,
            raw: trimmed.to_string(),
            line: index + 1,
        });
    }
    Ok(unknowns)
}

/// `~~Write-path placement~~ **Resolved 2026-08-23** → D-0012: ...`
fn parse_resolution(id: &str, question: &str) -> Option<Resolution> {
    let marker = question.find("**Resolved")?;
    let tail = &question[marker + "**Resolved".len()..];
    let candidate: String = tail.trim_start().chars().take(10).collect();
    let date = if candidate.chars().all(|c| c.is_ascii_digit() || c == '-') {
        candidate
    } else {
        String::new()
    };
    // The settling record is the first decision id named after the marker.
    let by = mentioned_ids(tail, id).into_iter().find(|m| !m.starts_with('U'));
    Some(Resolution { date, by })
}

/// The register states its own owner in a footer; the gaps it yields are
/// authored by that person. Reading it from the document beats hardcoding a
/// name into the ingester.
pub fn register_owner(text: &str) -> Option<String> {
    for line in text.lines() {
        let Some(ix) = line.find("Owner: ") else { continue };
        let name = line[ix + "Owner: ".len()..].trim().trim_end_matches(['*', '.', ' ']).trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    None
}

/// Open rows whose own trigger names a question that has since been
/// resolved — the register checked by the machinery that already checks the
/// golden questions, because two rows sat with fired triggers for days this
/// week (U-11, U-12) while the suite's questions could not have (D-0052).
///
/// Mechanical honesty about scope: a trigger written as prose ("data-model
/// implementation") is invisible here, and this observes only the subset
/// that names ids. The quarterly re-read still owns the rest; this owns what
/// a build can own.
pub fn stale_unknown_triggers(unknowns: &[ParsedUnknown]) -> Vec<(String, String)> {
    unknowns
        .iter()
        .filter(|row| row.resolved.is_none())
        .filter_map(|row| {
            let named = mentioned_ids(&row.trigger, &row.id);
            let fired = named.iter().find(|id| {
                is_unknown_id(id)
                    && unknowns.iter().any(|u| u.id == **id && u.resolved.is_some())
            })?;
            Some((row.id.clone(), format!("{fired} is resolved: \"{}\"", row.trigger)))
        })
        .collect()
}

/// Promoted decisions whose review trigger names a question that has since
/// been resolved. The same check, aimed at Room 1: a decision's trigger is
/// its promise to be re-read, and rewording the trigger after the re-read is
/// the acknowledgment that clears the alarm — which supersedes the record
/// through the ordinary sync, exactly as any edit does (D-0021).
pub fn stale_decision_triggers(
    records: &[crate::parse::ParsedRecord],
    unknowns: &[ParsedUnknown],
) -> Vec<(String, String)> {
    records
        .iter()
        .filter(|record| record.yaml.get("state").is_some_and(|s| s == "promoted"))
        .filter_map(|record| {
            let trigger = record.yaml.get("review_trigger")?;
            let named = mentioned_ids(trigger, &record.id);
            let fired = named.iter().find(|id| {
                is_unknown_id(id)
                    && unknowns.iter().any(|u| u.id == **id && u.resolved.is_some())
            })?;
            Some((record.id.clone(), format!("{fired} is resolved: \"{trigger}\"")))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_open_and_resolved_rows() {
        let doc = "\
## Room 2

| id | Question | Trigger | Notes |
|----|----------|---------|-------|
| U-1 | ~~Write-path placement~~ **Resolved 2026-08-23** → D-0012: grammar in the engine | — | Kept for the record. |
| U-5 | Storage layer: build vs embed | Implementation phase | An event-log design remains a candidate. |
";
        let parsed = parse_register(doc).unwrap();
        assert_eq!(parsed.len(), 2);

        let resolved = parsed[0].resolved.clone().unwrap();
        assert_eq!(resolved.date, "2026-08-23");
        assert_eq!(resolved.by.as_deref(), Some("D-0012"));

        assert!(parsed[1].resolved.is_none());
        assert_eq!(parsed[1].trigger, "Implementation phase");
    }

    /// D-0052's checks, on fixtures: an open row or a promoted decision whose
    /// trigger names a resolved question is flagged; naming the resolving
    /// decision instead is the acknowledgment and stays quiet.
    #[test]
    fn a_trigger_naming_a_resolved_question_is_flagged_until_reworded() {
        let register = "\
| id | Question | Trigger | Notes |
|----|----------|---------|-------|
| U-1 | ~~settled~~ **Resolved 2026-08-23** → D-0012: done | — | kept |
| U-2 | still open, waiting on the settled one | with U-1 | notes |
| U-3 | open, acknowledged properly | re-read when D-0012 landed | notes |

*Recorded 2026-08-23. Owner: Greg Villa.*
";
        let unknowns = parse_register(register).unwrap();
        let stale = stale_unknown_triggers(&unknowns);
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].0, "U-2");
        assert!(stale[0].1.contains("U-1 is resolved"));

        let decisions = "\
---

## D-0001 · A promoted decision resting on the resolved

```yaml
id: D-0001
state: promoted
author: Greg Villa
recorded: 2026-08-23
valid_from: 2026-08-23
source: test
review_trigger: revisit when U-1 resolves
```

**Assertion.** Something the suite needs to watch.

---

## D-0002 · One that acknowledged

```yaml
id: D-0002
state: promoted
author: Greg Villa
recorded: 2026-08-23
valid_from: 2026-08-23
source: test
review_trigger: re-read when D-0012 settled things; still live otherwise
```

**Assertion.** Quiet, because the re-read happened and says so.
";
        let records = crate::parse::parse_corpus(decisions).unwrap();
        let stale = stale_decision_triggers(&records, &unknowns);
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].0, "D-0001");
    }

    /// The live gate: this repository's own register and decisions carry no
    /// fired trigger. The first run of this check found one row and fourteen
    /// decisions in arrears — some fired that same week, one eight days old —
    /// and every one was re-read and acknowledged in the change that added
    /// the check (D-0052), which is the only honest way to give an alarm its
    /// first day.
    #[test]
    fn no_trigger_sits_fired_in_this_repository() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let register = std::fs::read_to_string(root.join("docs/REGISTER.md")).unwrap();
        let unknowns = parse_register(&register).unwrap();
        let rows = stale_unknown_triggers(&unknowns);
        assert!(rows.is_empty(), "open rows waiting on resolved questions: {rows:?}");
        let decisions = std::fs::read_to_string(root.join("docs/DECISIONS.md")).unwrap();
        let records = crate::parse::parse_corpus(&decisions).unwrap();
        let stale = stale_decision_triggers(&records, &unknowns);
        assert!(stale.is_empty(), "promoted decisions resting on fired triggers: {stale:?}");
    }

    #[test]
    fn a_malformed_row_is_a_hard_error() {
        let doc = "| U-9 | only two cells |\n";
        assert!(matches!(parse_register(doc), Err(ParseError::BadRegisterRow { .. })));
    }

    #[test]
    fn duplicate_ids_are_a_hard_error() {
        let doc = "\
| U-3 | a | t | n |
| U-3 | b | t | n |
";
        assert!(matches!(parse_register(doc), Err(ParseError::DuplicateUnknown { .. })));
    }

    #[test]
    fn mentions_span_every_cell() {
        let doc = "| U-20 | Set verdicts | With U-16 | Evidence for D-0004. |\n";
        let parsed = parse_register(doc).unwrap();
        assert_eq!(parsed[0].mentions(), vec!["U-16", "D-0004"]);
    }
}

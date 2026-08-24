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

    for line in text.lines() {
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

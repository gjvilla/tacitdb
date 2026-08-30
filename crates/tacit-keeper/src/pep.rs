//! A corpus in real language, written by people who never heard of this engine.
//!
//! The self-hosting corpus cannot grade retrieval honestly, because it
//! describes its own grading (U-27). The generated one (D-0030) fixes that
//! structurally and gives up real language to do it: synthetic prose has no
//! paraphrase, no dialect and no jargon drift, so it measures ranking and cost
//! and cannot measure the thing U-23 is about. This is the other half of U-9.
//!
//! Python enhancement proposals are the source, for reasons that are about
//! their shape rather than their subject. They are dual public-domain and
//! CC0-1.0, so vendoring raises no question. And their headers carry author,
//! dates, a status over nine values and supersession in both directions — this
//! engine's own model as document metadata rather than prose convention, which
//! is more than `docs/DECISIONS.md` manages. Nothing here needs a Python
//! interpreter: the documents are text, and this reads them.
//!
//! **What this deliberately does not do yet.** It vendors no documents. The
//! adapter takes text and a caller supplies it, so the questions of how many
//! proposals to pin and where they live stay open, and no real person's
//! contact details enter this repository while U-11 is unresolved.

use crate::corpus::IngestError;
use std::collections::BTreeMap;
use tacit_core::{
    Author, ClaimContent, Content, Draft, EntityId, Ledger, RecordId, RetireReason, SourceRef,
    Value, VerdictAction, VerdictContent,
};

/// The entity kind proposals are filed under, so a view can ask for them
/// without knowing which corpus they came from.
pub const PROPOSAL_KIND: &str = "proposal";
/// The predicate a proposal's stated dependency becomes.
pub const REQUIRES: &str = "requires";

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum PepError {
    #[error("proposal {number}: missing required header {key:?}")]
    MissingHeader { number: String, key: String },

    #[error("proposal {number}: unknown status {status:?}")]
    UnknownStatus { number: String, status: String },

    #[error("proposal {number}: header {key:?} is not a proposal number: {value:?}")]
    NotANumber { number: String, key: String, value: String },

    #[error("proposal {number}: no body beneath the headers")]
    EmptyBody { number: String },

    #[error("a header line before any key: {line:?}")]
    OrphanContinuation { line: String },

    #[error("no headers at all")]
    NoHeaders,
}

/// The nine values the status header takes.
///
/// Kept as an enum rather than a string because the lifecycle mapping below is
/// the whole point of the adapter, and a status nobody has mapped must be a
/// compile error or a parse error — never a silent default. That is the same
/// rule the decision-record parser follows: a corpus about honesty must not
/// drop what it does not understand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Draft,
    Active,
    Accepted,
    Provisional,
    Deferred,
    Rejected,
    Withdrawn,
    Final,
    Superseded,
}

impl Status {
    fn parse(number: &str, raw: &str) -> Result<Self, PepError> {
        Ok(match raw.trim() {
            "Draft" => Self::Draft,
            "Active" => Self::Active,
            "Accepted" => Self::Accepted,
            "Provisional" => Self::Provisional,
            "Deferred" => Self::Deferred,
            "Rejected" => Self::Rejected,
            "Withdrawn" => Self::Withdrawn,
            "Final" => Self::Final,
            "Superseded" => Self::Superseded,
            other => {
                return Err(PepError::UnknownStatus {
                    number: number.to_string(),
                    status: other.to_string(),
                });
            }
        })
    }

    /// What this status means in the engine's lifecycle.
    ///
    /// Six of the nine map cleanly. Two do not, and are marked so the count of
    /// records resting on a judgement call is reportable rather than buried:
    ///
    /// - **Provisional** is acceptance with a stated reservation. Read as
    ///   promoted, because the proposal governs while it holds — but the
    ///   reservation has nowhere to live, and a promoted record that says
    ///   "for now" is not a thing this engine can express.
    /// - **Deferred** is nobody deciding. Read as proposed, which is right
    ///   about the state and wrong about the intent: a draft nobody has read
    ///   and a draft everybody has read and set aside are the same record here.
    fn lifecycle(self) -> Lifecycle {
        match self {
            Self::Draft => Lifecycle::Proposed,
            Self::Deferred => Lifecycle::Proposed,
            Self::Accepted | Self::Active | Self::Final => Lifecycle::Promoted,
            Self::Provisional => Lifecycle::Promoted,
            Self::Rejected | Self::Withdrawn => Lifecycle::Refused,
            Self::Superseded => Lifecycle::Replaced,
        }
    }

    /// Whether reading this status cost a judgement the source did not make.
    fn is_judged(self) -> bool {
        matches!(self, Self::Provisional | Self::Deferred)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lifecycle {
    Proposed,
    Promoted,
    Refused,
    Replaced,
}

/// One proposal, parsed and not yet in a ledger.
#[derive(Debug, Clone, PartialEq)]
pub struct Pep {
    pub number: u32,
    pub title: String,
    /// Author names with any address stripped. See `strip_address`.
    pub authors: Vec<String>,
    pub status: Status,
    pub kind: String,
    pub created: Option<String>,
    /// Proposals this one replaces, from the `Replaces` header.
    pub replaces: Vec<u32>,
    /// The proposal that replaced this one, from `Superseded-By`.
    pub superseded_by: Option<u32>,
    pub requires: Vec<u32>,
    /// Where the decision was announced, when the header says.
    pub resolution: Option<String>,
    pub body: String,
}

impl Pep {
    pub fn label(&self) -> String {
        format!("PEP-{:04}", self.number)
    }
}

/// What an ingest built, and what it had to decide on the way.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct PepReport {
    pub proposals: usize,
    pub records: usize,
    pub promoted: usize,
    pub proposed: usize,
    pub refused: usize,
    pub retired: usize,
    /// Records whose state rests on reading `Provisional` or `Deferred` —
    /// the two statuses the engine has no home for.
    pub judged: Vec<String>,
    /// Supersession and dependency headers naming a proposal the caller did
    /// not supply. Reported rather than dropped: a link that points nowhere is
    /// a fact about the slice, not an error in the source.
    pub dangling: Vec<(String, String)>,
    pub entities: BTreeMap<u32, EntityId>,
}

/// Split a proposal into its header block and its body.
///
/// The format is RFC-2822: `Key: value`, a continuation line indented, and the
/// first blank line ends the block.
fn split_headers(text: &str) -> Result<(Vec<(String, String)>, String), PepError> {
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.peek() {
        if line.trim().is_empty() {
            lines.next();
            break;
        }
        let line = lines.next().expect("peeked");
        if line.starts_with([' ', '\t']) {
            let Some((_, value)) = headers.last_mut() else {
                return Err(PepError::OrphanContinuation { line: line.trim().to_string() });
            };
            value.push(' ');
            value.push_str(line.trim());
            continue;
        }
        match line.split_once(':') {
            Some((key, value)) => headers.push((key.trim().to_string(), value.trim().to_string())),
            None => {
                return Err(PepError::OrphanContinuation { line: line.trim().to_string() });
            }
        }
    }
    if headers.is_empty() {
        return Err(PepError::NoHeaders);
    }
    Ok((headers, lines.collect::<Vec<_>>().join("\n")))
}

/// `A Nother <a.nother@example.org>` becomes `A Nother`.
///
/// The address is dropped rather than stored. U-11 — a designed removal that
/// preserves chain integrity — is open, and its trigger is any external or
/// personal-data corpus. Names are what attribution needs (R-6); contact
/// details are what U-11 is about. Keeping only the first costs nothing the
/// provenance chain uses.
fn strip_address(entry: &str) -> String {
    entry.split('<').next().unwrap_or(entry).trim().trim_end_matches(',').to_string()
}

fn numbers(number: &str, key: &str, raw: &str) -> Result<Vec<u32>, PepError> {
    raw.split([',', ' '])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<u32>().map_err(|_| PepError::NotANumber {
                number: number.to_string(),
                key: key.to_string(),
                value: s.to_string(),
            })
        })
        .collect()
}

/// Parse one proposal.
pub fn parse_pep(text: &str) -> Result<Pep, PepError> {
    let (headers, body) = split_headers(text)?;
    let get = |key: &str| -> Option<&str> {
        headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.as_str())
    };
    // The number names the proposal in every error below, so it is read first
    // and reported as `?` when it is the missing thing.
    let raw_number = get("PEP").unwrap_or("?");
    let required = |key: &str| -> Result<&str, PepError> {
        get(key).ok_or_else(|| PepError::MissingHeader {
            number: raw_number.to_string(),
            key: key.to_string(),
        })
    };

    let number_text = required("PEP")?;
    let number = number_text.parse::<u32>().map_err(|_| PepError::NotANumber {
        number: raw_number.to_string(),
        key: "PEP".into(),
        value: number_text.to_string(),
    })?;
    let title = required("Title")?.to_string();
    let status = Status::parse(raw_number, required("Status")?)?;
    let kind = required("Type")?.to_string();
    let authors: Vec<String> = required("Author")?
        .split(',')
        .map(strip_address)
        .filter(|a| !a.is_empty())
        .collect();
    if authors.is_empty() {
        return Err(PepError::MissingHeader {
            number: raw_number.to_string(),
            key: "Author".into(),
        });
    }

    let replaces = get("Replaces").map(|v| numbers(raw_number, "Replaces", v)).transpose()?;
    let superseded = get("Superseded-By")
        .map(|v| numbers(raw_number, "Superseded-By", v))
        .transpose()?
        .and_then(|v| v.first().copied());
    let requires = get("Requires").map(|v| numbers(raw_number, "Requires", v)).transpose()?;

    if body.trim().is_empty() {
        return Err(PepError::EmptyBody { number: raw_number.to_string() });
    }

    Ok(Pep {
        number,
        title,
        authors,
        status,
        kind,
        created: get("Created").map(str::to_string),
        replaces: replaces.unwrap_or_default(),
        superseded_by: superseded,
        requires: requires.unwrap_or_default(),
        resolution: get("Resolution").map(str::to_string),
        body,
    })
}

/// Who declares the verdicts this adapter transcribes.
///
/// Not the proposal's author. A proposal's status is set by its editors and
/// deciding body, announced at the `Resolution` link — so attributing the
/// promotion to whoever typed the document would name the wrong person, which
/// is the fault D-0025 named when it made the transcriber distinct from the
/// author. The detail says where the reader should go to check.
fn decider(pep: &Pep) -> Author {
    Author {
        name: "proposal process".into(),
        kind: tacit_core::AuthorKind::Human,
        detail: Some(match &pep.resolution {
            Some(link) => format!("transcribed from {} Status: resolution {link}", pep.label()),
            None => format!("transcribed from {} Status: no resolution stated", pep.label()),
        }),
    }
}

fn source_ref(pep: &Pep) -> SourceRef {
    SourceRef { channel: "proposal".into(), reference: Some(pep.label()) }
}

fn verdict(author: &Author, pep: &Pep, action: VerdictAction, why: String) -> Draft {
    Draft::new(
        author.clone(),
        source_ref(pep),
        Content::Verdict(VerdictContent { action, rationale: Some(why) }),
    )
}

/// Build the ledger records for a slice of proposals.
///
/// Proposals are taken in the order given. A supersession or dependency naming
/// a proposal outside the slice is recorded in the report rather than dropped
/// or invented, because a partial corpus is the normal case and a link that
/// points outside it is a fact about the slice.
pub fn ingest_peps(ledger: &mut Ledger, peps: &[Pep]) -> Result<PepReport, IngestError> {
    let mut report = PepReport { proposals: peps.len(), ..PepReport::default() };
    let mut body_of: BTreeMap<u32, RecordId> = BTreeMap::new();
    let present: std::collections::BTreeSet<u32> = peps.iter().map(|p| p.number).collect();

    // Pass one lays down the claims. Verdicts cannot be written here: whether a
    // superseded proposal retires itself depends on whether its replacement is
    // in the slice at all, and that is not knowable until every claim exists.
    // The first version of this wrote both in one pass and the engine refused
    // it — a retired record was promoted again — which is invariant 3 catching
    // a modelling error rather than a typo.
    for pep in peps {
        let author = Author::human(pep.authors.join(", "));
        let subject = ledger.add_entity(PROPOSAL_KIND, pep.label())?;
        report.entities.insert(pep.number, subject);

        ledger.append(Draft::new(
            author.clone(),
            source_ref(pep),
            Content::Claim(ClaimContent::Attribute {
                subject,
                name: "title".into(),
                value: Value::Text(pep.title.clone()),
            }),
        ))?;
        report.records += 1;

        let mut body = Draft::new(
            author.clone(),
            source_ref(pep),
            Content::Claim(ClaimContent::Text { body: pep.body.clone(), about: vec![subject] }),
        );
        // Supersession lives on the superseding record (D-0023), and only when
        // what it replaces is in the slice.
        for replaced in &pep.replaces {
            match body_of.get(replaced) {
                Some(id) => body.supersedes = Some(*id),
                None => report.dangling.push((pep.label(), format!("Replaces {replaced}"))),
            }
        }
        let body_id = ledger.append(body)?;
        body_of.insert(pep.number, body_id);
        report.records += 1;

        if pep.status.is_judged() {
            report.judged.push(pep.label());
        }
    }

    // Pass two rules on them, with the whole slice visible.
    let mut promoted: std::collections::BTreeSet<RecordId> = std::collections::BTreeSet::new();
    for pep in peps {
        let author = Author::human(pep.authors.join(", "));
        let subject = report.entities[&pep.number];
        for required in &pep.requires {
            match report.entities.get(required) {
                Some(other) => {
                    ledger.append(Draft::new(
                        author.clone(),
                        source_ref(pep),
                        Content::Claim(ClaimContent::Relation {
                            subject,
                            predicate: REQUIRES.into(),
                            object: *other,
                            properties: Default::default(),
                        }),
                    ))?;
                    report.records += 1;
                }
                None => report.dangling.push((pep.label(), format!("Requires {required}"))),
            }
        }

        let decider = decider(pep);
        let body_id = body_of[&pep.number];
        // One verdict promotes this and retires what it replaces (design/001
        // §3.1) — but only a record already promoted can be retired, so a
        // replacement that arrives before the thing it replaces promotes alone
        // and says so rather than failing.
        let retiring = pep
            .replaces
            .iter()
            .find_map(|r| body_of.get(r).copied())
            .filter(|id| promoted.contains(id));
        if retiring.is_none()
            && let Some(r) = pep.replaces.first()
            && body_of.contains_key(r)
        {
            report.dangling.push((pep.label(), format!("Replaces {r} out of order")));
        }

        match pep.status.lifecycle() {
            Lifecycle::Proposed => report.proposed += 1,
            Lifecycle::Promoted => {
                ledger.append(verdict(
                    &decider,
                    pep,
                    VerdictAction::Promote { target: body_id, retiring },
                    format!("status {:?}", pep.status),
                ))?;
                promoted.insert(body_id);
                if let Some(id) = retiring {
                    promoted.remove(&id);
                    report.retired += 1;
                }
                report.records += 1;
                report.promoted += 1;
            }
            Lifecycle::Refused => {
                ledger.append(verdict(
                    &decider,
                    pep,
                    VerdictAction::Reject { target: body_id },
                    format!("status {:?}", pep.status),
                ))?;
                report.records += 1;
                report.refused += 1;
            }
            Lifecycle::Replaced => {
                ledger.append(verdict(
                    &decider,
                    pep,
                    VerdictAction::Promote { target: body_id, retiring },
                    format!("status {:?}", pep.status),
                ))?;
                promoted.insert(body_id);
                if let Some(id) = retiring {
                    promoted.remove(&id);
                    report.retired += 1;
                }
                report.records += 1;
                report.promoted += 1;

                // Its successor retires it if the successor is here; otherwise
                // the status is the only witness and this verdict is it.
                match pep.superseded_by {
                    Some(n) if present.contains(&n) => {}
                    other => {
                        ledger.append(verdict(
                            &decider,
                            pep,
                            VerdictAction::Retire {
                                target: body_id,
                                reason: RetireReason::Superseded,
                            },
                            match other {
                                Some(n) => format!("superseded by PEP-{n:04}, outside this slice"),
                                None => "superseded, successor unstated".into(),
                            },
                        ))?;
                        promoted.remove(&body_id);
                        report.records += 1;
                        report.retired += 1;
                        if let Some(n) = other {
                            report.dangling.push((pep.label(), format!("Superseded-By {n}")));
                        }
                    }
                }
            }
        }
    }
    Ok(report)
}


#[cfg(test)]
mod tests {
    use super::*;

    const ZEN: &str = "PEP: 20\n\
        Title: A short informational note\n\
        Author: A Nother <a.nother@example.invalid>\n\
        Status: Active\n\
        Type: Informational\n\
        Created: 19-Aug-2004\n\
        \n\
        Abstract\n\
        ========\n\
        \n\
        Some aphorisms, of which not all are written down.\n";

    fn pep(number: u32, status: &str, extra: &str) -> String {
        format!(
            "PEP: {number}\nTitle: A proposal\nAuthor: A Person <a@example.invalid>\n\
             Status: {status}\nType: Standards Track\nCreated: 01-Jan-2001\n{extra}\n\
             \nBody\n====\n\nSome prose about the proposal.\n"
        )
    }

    #[test]
    fn headers_and_body_split_at_the_first_blank_line() {
        let parsed = parse_pep(ZEN).unwrap();
        assert_eq!(parsed.number, 20);
        assert_eq!(parsed.title, "A short informational note");
        assert_eq!(parsed.status, Status::Active);
        assert_eq!(parsed.created.as_deref(), Some("19-Aug-2004"));
        assert!(parsed.body.contains("aphorisms"));
        assert!(!parsed.body.contains("Status:"));
    }

    /// The address is the part U-11 is about, and it never reaches the record.
    #[test]
    fn an_author_keeps_a_name_and_loses_an_address() {
        let parsed = parse_pep(ZEN).unwrap();
        assert_eq!(parsed.authors, vec!["A Nother".to_string()]);
        assert!(!parsed.body.contains('@'));
    }

    #[test]
    fn a_continuation_line_extends_the_header_above_it() {
        let text = pep(1, "Draft", "Requires: 2,\n  3");
        let parsed = parse_pep(&text).unwrap();
        assert_eq!(parsed.requires, vec![2, 3]);
    }

    /// The rule the decision-record parser follows: a corpus about honesty does
    /// not silently drop what it cannot read.
    #[test]
    fn a_status_nobody_mapped_is_an_error_and_not_a_default() {
        let text = pep(9, "Percolating", "");
        assert_eq!(
            parse_pep(&text),
            Err(PepError::UnknownStatus { number: "9".into(), status: "Percolating".into() })
        );
    }

    #[test]
    fn a_missing_required_header_names_itself() {
        let text = ZEN.replace("Type: Informational\n", "");
        assert_eq!(
            parse_pep(&text),
            Err(PepError::MissingHeader { number: "20".into(), key: "Type".into() })
        );
    }

    #[test]
    fn headers_with_no_body_are_refused() {
        let text = "PEP: 3\nTitle: Hollow\nAuthor: A Person\nStatus: Draft\nType: Process\n\n";
        assert_eq!(parse_pep(text), Err(PepError::EmptyBody { number: "3".into() }));
    }

    #[test]
    fn a_final_proposal_is_promoted_and_a_draft_is_not() {
        let peps: Vec<Pep> = [pep(1, "Final", ""), pep(2, "Draft", "")]
            .iter()
            .map(|t| parse_pep(t).unwrap())
            .collect();
        let mut ledger = Ledger::new();
        let report = ingest_peps(&mut ledger, &peps).unwrap();
        assert_eq!(report.promoted, 1);
        assert_eq!(report.proposed, 1);
        assert_eq!(report.proposals, 2);
    }

    #[test]
    fn a_withdrawn_proposal_is_refused_rather_than_retired() {
        let peps = vec![parse_pep(&pep(4, "Withdrawn", "")).unwrap()];
        let mut ledger = Ledger::new();
        let report = ingest_peps(&mut ledger, &peps).unwrap();
        assert_eq!(report.refused, 1);
        assert_eq!(report.retired, 0);
    }

    /// Supersession lives on the superseding record, so the replacement has to
    /// arrive after what it replaces for the link to be made at all.
    #[test]
    fn a_replacement_links_to_what_it_replaces_when_both_are_present() {
        let peps: Vec<Pep> = [
            pep(10, "Superseded", "Superseded-By: 11"),
            pep(11, "Final", "Replaces: 10"),
        ]
        .iter()
        .map(|t| parse_pep(t).unwrap())
        .collect();
        let mut ledger = Ledger::new();
        let report = ingest_peps(&mut ledger, &peps).unwrap();
        assert_eq!(report.retired, 1);
        assert!(report.dangling.is_empty(), "both are present: {:?}", report.dangling);
    }

    /// A partial corpus is the normal case, and a link out of it is reported.
    #[test]
    fn a_link_out_of_the_slice_is_reported_and_not_invented() {
        let peps = vec![parse_pep(&pep(12, "Final", "Replaces: 999")).unwrap()];
        let mut ledger = Ledger::new();
        let report = ingest_peps(&mut ledger, &peps).unwrap();
        assert_eq!(report.dangling, vec![("PEP-0012".to_string(), "Replaces 999".to_string())]);
    }

    /// The two statuses the engine has no home for are counted, so the number
    /// of records resting on a judgement is answerable rather than buried.
    #[test]
    fn the_statuses_with_no_home_are_named_in_the_report() {
        let peps: Vec<Pep> = [pep(5, "Provisional", ""), pep(6, "Deferred", ""), pep(7, "Final", "")]
            .iter()
            .map(|t| parse_pep(t).unwrap())
            .collect();
        let mut ledger = Ledger::new();
        let report = ingest_peps(&mut ledger, &peps).unwrap();
        assert_eq!(report.judged, vec!["PEP-0005".to_string(), "PEP-0006".to_string()]);
        assert_eq!(report.promoted, 2, "provisional governs, deferred does not");
        assert_eq!(report.proposed, 1);
    }

    /// With its successor absent, the status is the only witness that the
    /// proposal stopped governing, so this verdict is it.
    #[test]
    fn a_superseded_proposal_alone_in_the_slice_retires_itself() {
        let peps = vec![parse_pep(&pep(13, "Superseded", "Superseded-By: 14")).unwrap()];
        let mut ledger = Ledger::new();
        let report = ingest_peps(&mut ledger, &peps).unwrap();
        assert_eq!(report.promoted, 1, "it governed before it was replaced");
        assert_eq!(report.retired, 1);
        assert_eq!(report.dangling, vec![("PEP-0013".to_string(), "Superseded-By 14".to_string())]);
    }

    /// The status is not the author's verdict, and the record says so.
    #[test]
    fn the_verdict_names_the_process_and_not_the_typist() {
        let peps = vec![parse_pep(&pep(8, "Final", "Resolution: https://example.invalid/x")).unwrap()];
        let mut ledger = Ledger::new();
        ingest_peps(&mut ledger, &peps).unwrap();
        let verdicts: Vec<&tacit_core::Record> = ledger
            .records()
            .filter(|r| matches!(r.content(), Content::Verdict(_)))
            .collect();
        assert_eq!(verdicts.len(), 1);
        let author = verdicts[0].envelope().author();
        assert_eq!(author.name, "proposal process");
        assert!(author.detail.as_deref().unwrap().contains("resolution"));
        assert_ne!(author.name, "A Person");
    }
}

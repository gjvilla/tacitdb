//! Ingest the decision-record corpus into a ledger.
//!
//! The dogfood: the file recording the project's decisions becomes the
//! engine's first corpus, and the engine's own grammar validates the file's
//! claims about itself. Nothing here bypasses the ratchet — the yaml `state:
//! promoted` is *transcribed* as a human-authored promote verdict, exactly as
//! if the author had rendered it at the keyboard, because that is what the
//! document records them doing.
//!
//! Bitemporal note: record-time is when this ledger learned the record (now,
//! engine-assigned), and the document's own date becomes valid-time. Nothing
//! is backdated and invariant 3 is untouched.

use crate::parse::{ParseError, ParsedRecord, mentioned_ids, parse_corpus, split_evidence};
use crate::register::{ParsedUnknown, parse_register, register_owner};
use jiff::civil::Date;
use jiff::tz::TimeZone;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tacit_core::{
    Author, ClaimContent, Content, Draft, EntityId, Evidence, GapContent, HypothesisContent,
    MemoryLedger, RecordId, ReviewTrigger, SourceRef, VerdictAction, VerdictContent,
};

/// Entity kind for a corpus record's identity anchor.
pub const DECISION_KIND: &str = "decision";
/// Entity kind for a register unknown's identity anchor.
pub const UNKNOWN_KIND: &str = "unknown";
/// Predicate for "this record's text names that record". A bare textual
/// mention is all that was observed, so the predicate claims nothing more.
pub const MENTIONS: &str = "mentions";

#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error(transparent)]
    Parse(#[from] ParseError),

    #[error(transparent)]
    Ledger(#[from] tacit_core::Error),

    #[error("io reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("record {record}: evidence {entry:?} resolves to no file under the repo root")]
    UnresolvableEvidence { record: String, entry: String },

    #[error("record {record}: evidence {entry:?} resolves outside the repo root")]
    EvidenceEscapesRepo { record: String, entry: String },

    #[error("record {record}: bad date {value:?} for key {key}")]
    BadDate { record: String, key: String, value: String },

    #[error("record {record}: unsupported corpus state {state:?}")]
    UnsupportedState { record: String, state: String },

    #[error("record {record}: id says hypothesis but sections say claim (or vice versa)")]
    HypothesisSignalMismatch { record: String },

    #[error("record {record}: score_by is only meaningful on a hypothesis")]
    StrayScoreBy { record: String },

    #[error("the register does not state an owner, so its gaps have no author")]
    MissingRegisterOwner,
}

/// What one ingest run put into the ledger.
#[derive(Debug, Default)]
pub struct IngestReport {
    pub sources: Vec<(String, EntityId)>,
    pub decisions: Vec<(String, EntityId)>,
    pub unknowns: Vec<(String, EntityId)>,
    pub gaps: Vec<(String, RecordId)>,
    /// Resolved unknowns, as (unknown id, the decision that settled it).
    pub answered: Vec<(String, String)>,
    pub content_claims: Vec<(String, RecordId)>,
    pub title_claims: Vec<(String, RecordId)>,
    pub mention_claims: Vec<(String, String, RecordId)>,
    pub verdicts: Vec<RecordId>,
    pub evidence_links: usize,
    /// Forces vectors the split heuristic produced, for the keeper to check.
    pub proposed_forces: Vec<(String, Vec<String>)>,
}

impl IngestReport {
    pub fn appended(&self) -> usize {
        self.content_claims.len()
            + self.title_claims.len()
            + self.mention_claims.len()
            + self.gaps.len()
            + self.verdicts.len()
    }

    pub fn decision(&self, id: &str) -> Option<EntityId> {
        self.decisions.iter().find(|(k, _)| k == id).map(|(_, e)| *e)
    }

    pub fn unknown(&self, id: &str) -> Option<EntityId> {
        self.unknowns.iter().find(|(k, _)| k == id).map(|(_, e)| *e)
    }

    /// An anchor of either kind — what a cross-reference resolves against.
    pub fn anchor(&self, id: &str) -> Option<EntityId> {
        self.decision(id).or_else(|| self.unknown(id))
    }

    pub fn gap(&self, id: &str) -> Option<RecordId> {
        self.gaps.iter().find(|(k, _)| k == id).map(|(_, r)| *r)
    }

    pub fn content_claim(&self, id: &str) -> Option<RecordId> {
        self.content_claims.iter().find(|(k, _)| k == id).map(|(_, r)| *r)
    }
}

/// Read and ingest `docs/DECISIONS.md` beneath `repo_root`.
pub fn ingest_decisions(
    ledger: &mut MemoryLedger,
    repo_root: &Path,
) -> Result<IngestReport, IngestError> {
    let decisions = read_doc(repo_root, "docs/DECISIONS.md")?;
    ingest_text(ledger, &decisions, None, repo_root)
}

/// Ingest both founding documents: the decision records and the register's
/// known unknowns. The register's open questions become gap records, which is
/// what lets the engine answer "that is a registered open question" rather
/// than "nothing found".
pub fn ingest_corpus(
    ledger: &mut MemoryLedger,
    repo_root: &Path,
) -> Result<IngestReport, IngestError> {
    let decisions = read_doc(repo_root, "docs/DECISIONS.md")?;
    let register = read_doc(repo_root, "docs/REGISTER.md")?;
    ingest_text(ledger, &decisions, Some(&register), repo_root)
}

fn read_doc(repo_root: &Path, relative: &str) -> Result<String, IngestError> {
    let path = repo_root.join(relative);
    std::fs::read_to_string(&path).map_err(|source| IngestError::Io { path, source })
}

pub fn ingest_text(
    ledger: &mut MemoryLedger,
    text: &str,
    register_text: Option<&str>,
    repo_root: &Path,
) -> Result<IngestReport, IngestError> {
    let parsed = parse_corpus(text)?;
    let unknowns = match register_text {
        Some(register) => parse_register(register)?,
        None => Vec::new(),
    };
    let register_author = match register_text {
        Some(register) => Some(Author::human(
            register_owner(register).ok_or(IngestError::MissingRegisterOwner)?,
        )),
        None => None,
    };
    let mut report = IngestReport::default();

    // Identity first, for both corpora, so a cross-reference resolves in
    // either direction: a decision naming U-1 and an unknown naming D-0012
    // both find an anchor.
    for record in &parsed {
        let entity = ledger.upsert_entity(DECISION_KIND, &record.id);
        report.decisions.push((record.id.clone(), entity));
    }
    for unknown in &unknowns {
        let entity = ledger.upsert_entity(UNKNOWN_KIND, &unknown.id);
        report.unknowns.push((unknown.id.clone(), entity));
    }

    // Phase 1 — every decision record lands proposed.
    for record in &parsed {
        ingest_one(ledger, record, repo_root, &mut report)?;
    }

    // Phase 2 — every register row lands as a registered gap.
    if let Some(author) = &register_author {
        for unknown in &unknowns {
            ingest_gap(ledger, unknown, author, &mut report)?;
        }
    }

    // Phase 3 — transcribe the verdicts the decision document records.
    for record in &parsed {
        let state = record.require("state")?;
        let author = Author::human(record.require("author")?);
        match state {
            "promoted" => {
                let targets = [
                    report.content_claim(&record.id),
                    report
                        .title_claims
                        .iter()
                        .find(|(k, _)| *k == record.id)
                        .map(|(_, r)| *r),
                ];
                for target in targets.into_iter().flatten() {
                    let verdict = ledger.append(Draft::new(
                        author.clone(),
                        SourceRef {
                            channel: "corpus-ingest".into(),
                            reference: Some(format!("docs/DECISIONS.md {} state:", record.id)),
                        },
                        Content::Verdict(VerdictContent {
                            action: VerdictAction::Promote { target, retiring: None },
                            rationale: Some(format!(
                                "transcribed from docs/DECISIONS.md: {} carries `state: promoted`",
                                record.id
                            )),
                        }),
                    ))?;
                    report.verdicts.push(verdict);
                }
            }
            // A hypothesis is never promoted — it is scored, and this one is
            // not yet due.
            "registered" => {}
            other => {
                return Err(IngestError::UnsupportedState {
                    record: record.id.clone(),
                    state: other.to_string(),
                });
            }
        }
    }

    // Phase 4 — transcribe the register's resolutions. Last, because the
    // engine refuses to answer a gap with a claim that is not yet promoted:
    // the ordering is forced by the grammar, not chosen for convenience.
    if let Some(author) = &register_author {
        for unknown in &unknowns {
            let Some(resolution) = &unknown.resolved else { continue };
            let gap = report.gap(&unknown.id).expect("gap appended in phase 2");
            let settled_by = resolution.by.as_ref().and_then(|d| report.content_claim(d));
            let (action, rationale) = match (settled_by, &resolution.by) {
                (Some(with_claim), Some(decision)) => (
                    VerdictAction::Answer { gap, with_claim },
                    format!(
                        "transcribed from docs/REGISTER.md: {} resolved {} by {decision}",
                        unknown.id, resolution.date
                    ),
                ),
                _ => (
                    VerdictAction::Withdraw { gap },
                    format!(
                        "transcribed from docs/REGISTER.md: {} resolved {} with no \
                         settling record named",
                        unknown.id, resolution.date
                    ),
                ),
            };
            let verdict = ledger.append(Draft::new(
                author.clone(),
                SourceRef {
                    channel: "corpus-ingest".into(),
                    reference: Some(format!("docs/REGISTER.md {}", unknown.id)),
                },
                Content::Verdict(VerdictContent { action, rationale: Some(rationale) }),
            ))?;
            report.verdicts.push(verdict);
            if let Some(decision) = &resolution.by {
                report.answered.push((unknown.id.clone(), decision.clone()));
            }
        }
    }

    Ok(report)
}

/// One register row becomes one gap: the question, the territory it covers,
/// and its trigger as the review trigger. The Notes column is carried into the
/// question rather than dropped — a register that loses its own commentary on
/// ingest would be a poor advertisement for a corpus about honesty.
fn ingest_gap(
    ledger: &mut MemoryLedger,
    unknown: &ParsedUnknown,
    author: &Author,
    report: &mut IngestReport,
) -> Result<(), IngestError> {
    let anchor = report.unknown(&unknown.id).expect("anchor minted above");
    let mut territory = vec![anchor];
    for mention in unknown.mentions() {
        if let Some(entity) = report.anchor(&mention) {
            territory.push(entity);
        }
    }

    let question = if unknown.notes.is_empty() {
        unknown.question.clone()
    } else {
        format!("{}\n\nNotes. {}", unknown.question, unknown.notes)
    };

    let trigger = unknown.trigger.trim();
    let review_trigger = (!trigger.is_empty() && trigger != "—").then(|| ReviewTrigger {
        due_at: None,
        on_event: Some(trigger.to_string()),
    });

    let mut draft = Draft::new(
        author.clone(),
        SourceRef {
            channel: "register".into(),
            reference: Some(format!("docs/REGISTER.md {}", unknown.id)),
        },
        Content::Gap(GapContent { question, territory }),
    );
    draft.review_trigger = review_trigger;
    let id = ledger.append(draft)?;
    report.gaps.push((unknown.id.clone(), id));
    Ok(())
}

fn ingest_one(
    ledger: &mut MemoryLedger,
    record: &ParsedRecord,
    repo_root: &Path,
    report: &mut IngestReport,
) -> Result<(), IngestError> {
    let subject = report.decision(&record.id).expect("anchor minted above");
    let author = Author::human(record.require("author")?);
    let source = SourceRef {
        channel: record.require("source")?.to_string(),
        // The document's own stated record-time, preserved verbatim rather
        // than backdating the ledger's.
        reference: Some(format!(
            "docs/DECISIONS.md {} recorded:{}",
            record.id,
            record.yaml.get("recorded").map(String::as_str).unwrap_or("—")
        )),
    };
    let valid_from = parse_date(record, "valid_from")?;
    let review_trigger = record.yaml.get("review_trigger").map(|prose| ReviewTrigger {
        due_at: None,
        on_event: Some(prose.clone()),
    });

    let evidence = resolve_evidence(ledger, record, repo_root, report)?;
    report.evidence_links += evidence.len();

    let content = build_content(record, subject)?;
    if let Content::Claim(ClaimContent::Pattern { forces, .. }) = &content {
        report.proposed_forces.push((record.id.clone(), forces.clone()));
    }

    let mut draft = Draft::new(author.clone(), source.clone(), content);
    draft.valid_from = valid_from;
    draft.evidence = evidence;
    draft.review_trigger = review_trigger.clone();
    let content_claim = ledger.append(draft)?;
    report.content_claims.push((record.id.clone(), content_claim));

    // The heading title, transcribed verbatim as a node property.
    let mut title = Draft::new(
        author.clone(),
        source.clone(),
        Content::Claim(ClaimContent::Attribute {
            subject,
            name: "title".into(),
            value: tacit_core::Value::Text(record.title.clone()),
        }),
    );
    title.valid_from = valid_from;
    let title_claim = ledger.append(title)?;
    report.title_claims.push((record.id.clone(), title_claim));

    // Cross-references, as observed: this record's text names that record.
    // These stay *proposed* — no human has ratified the machine's reading, so
    // the default graph shows no edges until someone does.
    for other in mentioned_ids(&record.raw, &record.id) {
        let Some(object) = report.anchor(&other) else { continue };
        let mut edge = Draft::new(
            Author::agent("corpus-ingest"),
            source.clone(),
            Content::Claim(ClaimContent::Relation {
                subject,
                predicate: MENTIONS.into(),
                object,
                properties: BTreeMap::new(),
            }),
        );
        edge.valid_from = valid_from;
        let id = ledger.append(edge)?;
        report.mention_claims.push((record.id.clone(), other, id));
    }

    Ok(())
}

fn build_content(record: &ParsedRecord, subject: EntityId) -> Result<Content, IngestError> {
    let is_hypothesis_id = record.id.starts_with('H');
    let has_hypothesis_section = record.section("Hypothesis").is_some();
    if is_hypothesis_id != has_hypothesis_section {
        return Err(IngestError::HypothesisSignalMismatch { record: record.id.clone() });
    }

    if is_hypothesis_id {
        let score_by = parse_date(record, "score_by")?.ok_or_else(|| IngestError::BadDate {
            record: record.id.clone(),
            key: "score_by".into(),
            value: String::new(),
        })?;
        return Ok(Content::Hypothesis(HypothesisContent {
            statement: record.section("Hypothesis").expect("checked").to_string(),
            falsifier: record.section("Falsifier").map(str::to_string),
            score_by,
        }));
    }
    if record.yaml.contains_key("score_by") {
        return Err(IngestError::StrayScoreBy { record: record.id.clone() });
    }

    let assertion = record.section("Assertion").unwrap_or_default();
    let trailing = |skip: &[&str]| -> String {
        record
            .sections
            .iter()
            .filter(|(label, _)| !skip.contains(&label.as_str()))
            .map(|(label, body)| format!("\n\n{label}. {body}"))
            .collect()
    };

    Ok(match record.section("Forces") {
        Some(forces) => Content::Claim(ClaimContent::Pattern {
            context: record.title.clone(),
            forces: split_forces(forces),
            solution: format!("{assertion}{}", trailing(&["Assertion", "Forces"])),
            about: vec![subject],
        }),
        None => Content::Claim(ClaimContent::Text {
            body: format!("{assertion}{}", trailing(&["Assertion"])),
            about: vec![subject],
        }),
    })
}

/// Semicolons are the author's own force separator. Sentence-splitting would
/// mis-fire on clauses ending in a quoted phrase, so this stays deliberately
/// crude — and the ingest reports what it produced, because the split is the
/// machine proposing, not deciding.
fn split_forces(paragraph: &str) -> Vec<String> {
    let parts: Vec<String> = paragraph
        .split("; ")
        .map(|p| p.trim().trim_end_matches('.').trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();
    if parts.is_empty() { vec![paragraph.trim().to_string()] } else { parts }
}

fn parse_date(
    record: &ParsedRecord,
    key: &str,
) -> Result<Option<jiff::Timestamp>, IngestError> {
    let Some(raw) = record.yaml.get(key) else { return Ok(None) };
    let value = raw.split_whitespace().next().unwrap_or(raw);
    let date: Date = value.parse().map_err(|_| IngestError::BadDate {
        record: record.id.clone(),
        key: key.to_string(),
        value: value.to_string(),
    })?;
    let stamp = date
        .to_zoned(TimeZone::UTC)
        .map_err(|_| IngestError::BadDate {
            record: record.id.clone(),
            key: key.to_string(),
            value: value.to_string(),
        })?
        .timestamp();
    Ok(Some(stamp))
}

/// Resolve each evidence entry to a source entity, hard-erroring on anything
/// that does not exist on disk or escapes the repo. Labels are stored
/// repo-relative: absolute paths would carry the author's home directory into
/// the corpus.
fn resolve_evidence(
    ledger: &mut MemoryLedger,
    record: &ParsedRecord,
    repo_root: &Path,
    report: &mut IngestReport,
) -> Result<Vec<Evidence>, IngestError> {
    let Some(list) = record.yaml.get("evidence") else { return Ok(Vec::new()) };
    let mut links = Vec::new();

    for entry in split_evidence(list) {
        let (raw_path, span) = split_entry(&entry);
        let relative = resolve_path(&raw_path, repo_root).ok_or_else(|| {
            IngestError::UnresolvableEvidence {
                record: record.id.clone(),
                entry: entry.clone(),
            }
        })?;
        if relative.starts_with("..") {
            return Err(IngestError::EvidenceEscapesRepo {
                record: record.id.clone(),
                entry: entry.clone(),
            });
        }
        let source = ledger.upsert_entity(tacit_core::SOURCE_KIND, &relative);
        if !report.sources.iter().any(|(label, _)| *label == relative) {
            report.sources.push((relative.clone(), source));
        }
        links.push(Evidence { source, span });
    }
    Ok(links)
}

/// `design/001-data-model.md §3` → (path, span). `this file — ...` is the
/// corpus referring to itself.
fn split_entry(entry: &str) -> (String, Option<String>) {
    if let Some(rest) = entry.strip_prefix("this file") {
        let span = rest.trim_start_matches([' ', '—']).trim();
        return (
            "DECISIONS.md".to_string(),
            (!span.is_empty()).then(|| span.to_string()),
        );
    }
    match entry.split_once(' ') {
        Some((path, span)) => (path.to_string(), Some(span.trim().to_string())),
        None => (entry.to_string(), None),
    }
}

/// Try `docs/<path>` then `<path>`, both under the repo root. Returns the
/// repo-relative form.
///
/// Absolute candidates are rejected before anything else: `Path::join`
/// *discards its base* when the argument is absolute, so `docs`.join("/etc/x")
/// is `/etc/x` — which would defeat both the prefix and the containment check
/// and store an absolute path as a source label. Containment is then confirmed
/// against canonical paths so a symlink cannot walk out of the repo either.
fn resolve_path(candidate: &str, repo_root: &Path) -> Option<String> {
    let cleaned = candidate.trim_end_matches('/');
    let as_path = Path::new(cleaned);
    if cleaned.is_empty() || as_path.is_absolute() {
        return None;
    }
    if as_path.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
        return None;
    }

    let root = repo_root.canonicalize().ok()?;
    for prefix in ["docs", ""] {
        let relative =
            if prefix.is_empty() { as_path.to_path_buf() } else { Path::new(prefix).join(as_path) };
        let full = repo_root.join(&relative);
        if !full.exists() {
            continue;
        }
        if !full.canonicalize().ok()?.starts_with(&root) {
            return None;
        }
        return Some(relative.to_string_lossy().replace('\\', "/"));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tacit_core::{ClaimState, Projection, RecordState, StateFilter, ViewSpec};

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn ingested() -> (MemoryLedger, IngestReport) {
        let mut ledger = MemoryLedger::new();
        let report = ingest_decisions(&mut ledger, &repo_root()).expect("corpus ingests");
        (ledger, report)
    }

    /// The document is expected to grow. These tests derive what to expect
    /// from an independent parse rather than freezing a census of it — a
    /// hardcoded count would turn every new decision record into a red suite.
    fn parsed() -> Vec<crate::parse::ParsedRecord> {
        let text = std::fs::read_to_string(repo_root().join("docs/DECISIONS.md")).unwrap();
        crate::parse::parse_corpus(&text).unwrap()
    }

    #[test]
    fn the_whole_corpus_ingests() {
        let (ledger, report) = ingested();
        let records = parsed();
        assert!(records.len() >= 16, "the parser found the corpus, not a fragment");

        assert_eq!(report.decisions.len(), records.len(), "one anchor per record");
        assert_eq!(report.content_claims.len(), records.len());
        assert_eq!(report.title_claims.len(), records.len());

        // Two verdicts per promoted record: its content claim and its title.
        let promoted = records.iter().filter(|r| r.yaml["state"] == "promoted").count();
        assert_eq!(report.verdicts.len(), promoted * 2);

        // Exactly one hypothesis, and it is the registered one.
        let hypotheses = records.iter().filter(|r| r.id.starts_with('H')).count();
        assert_eq!(hypotheses, records.len() - promoted);

        assert_eq!(ledger.log().len(), report.appended(), "nothing appended off-report");
    }

    #[test]
    fn evidence_resolves_to_real_files_only() {
        let (ledger, report) = ingested();
        let expected: usize = parsed()
            .iter()
            .filter_map(|r| r.yaml.get("evidence"))
            .map(|list| crate::parse::split_evidence(list).len())
            .sum();
        assert_eq!(report.evidence_links, expected);
        assert!(report.evidence_links >= 10);

        for (label, id) in &report.sources {
            assert!(!label.starts_with('/'), "labels stay repo-relative: {label}");
            assert!(!label.contains(".."), "labels never escape the repo: {label}");
            assert!(repo_root().join(label).exists(), "{label} exists on disk");
            assert_eq!(ledger.entity(*id).unwrap().kind(), tacit_core::SOURCE_KIND);
        }
        // The corpus's self-reference resolves to the corpus.
        assert!(report.sources.iter().any(|(l, _)| l == "docs/DECISIONS.md"));
        // Sources are deduplicated: several records cite the same design doc.
        let unique: std::collections::BTreeSet<_> =
            report.sources.iter().map(|(l, _)| l.clone()).collect();
        assert_eq!(unique.len(), report.sources.len());
        assert!(report.sources.len() < report.evidence_links, "dedup actually happened");
    }

    #[test]
    fn promoted_records_reach_promoted_only_through_verdicts() {
        let (ledger, report) = ingested();
        let d1 = report.content_claim("D-0001").unwrap();
        assert_eq!(ledger.state_of(d1), Some(RecordState::Claim(ClaimState::Promoted)));
        assert_eq!(ledger.history(d1).len(), 1);
        let verdict = ledger.history(d1)[0];
        assert_eq!(verdict.envelope().author().kind, tacit_core::AuthorKind::Human);
    }

    #[test]
    fn the_hypothesis_is_registered_not_promoted() {
        let (ledger, report) = ingested();
        let h = report.content_claim("H-0001").unwrap();
        assert_eq!(
            ledger.state_of(h),
            Some(RecordState::Hypothesis(tacit_core::HypothesisState::Registered))
        );
        assert!(ledger.history(h).is_empty());
        let Content::Hypothesis(content) = ledger.record(h).unwrap().content() else {
            panic!("H-0001 is a hypothesis");
        };
        assert!(content.falsifier.is_some());
        assert_eq!(content.score_by.strftime("%Y-%m-%d").to_string(), "2027-02-23");
    }

    /// The content-shape rule is mechanical: a record becomes a Pattern
    /// exactly when it carries a Forces section.
    #[test]
    fn forces_bearing_records_become_patterns() {
        let (ledger, report) = ingested();
        let expected: Vec<String> = parsed()
            .iter()
            .filter(|r| r.section("Forces").is_some())
            .map(|r| r.id.clone())
            .collect();
        assert!(expected.len() >= 5, "the corpus still has forces-bearing records");

        let mut patterns = Vec::new();
        for (id, claim) in &report.content_claims {
            if let Content::Claim(ClaimContent::Pattern { context, forces, about, .. }) =
                ledger.record(*claim).unwrap().content()
            {
                patterns.push(id.clone());
                assert!(!context.is_empty(), "{id} pattern has a context");
                assert!(!forces.is_empty(), "{id} pattern has forces");
                assert_eq!(about.len(), 1, "{id} is about its own anchor");
            }
        }
        assert_eq!(patterns, expected);
    }

    /// The machine's reading of cross-references is proposed, never promoted:
    /// the default graph has no edges until a human ratifies them.
    #[test]
    fn mention_edges_stay_proposed() {
        let (ledger, report) = ingested();
        assert!(!report.mention_claims.is_empty());
        for (_, _, id) in &report.mention_claims {
            assert_eq!(ledger.state_of(*id), Some(RecordState::Claim(ClaimState::Proposed)));
        }

        let projection = Projection::rebuild(&ledger);
        assert!(projection.view(&ledger, ViewSpec::now()).edges().is_empty());
        let proposed = projection
            .view(&ledger, ViewSpec::now().with_states(StateFilter::PromotedAndProposed));
        assert_eq!(proposed.edges().len(), report.mention_claims.len());
    }

    #[test]
    fn the_projected_graph_carries_titles_and_prose() {
        let (ledger, report) = ingested();
        let projection = Projection::rebuild(&ledger);
        let view = projection.view(&ledger, ViewSpec::now());

        let d12 = report.decision("D-0012").unwrap();
        let node = view.node(d12).unwrap();
        let title = node.property("title").unwrap();
        assert_eq!(
            title.single().unwrap().value(),
            &tacit_core::Value::Text("Write-path: grammar in the engine, truth in the keeper".into())
        );
        assert_eq!(node.about().len(), 1, "its own promoted content claim");

        // H-0001's anchor is dark in the default view: nothing about it is
        // promoted, because a hypothesis is scored rather than promoted.
        let h = report.decision("H-0001").unwrap();
        assert!(view.node(h).unwrap().property("title").is_none());
        assert!(view.node(h).unwrap().about().is_empty());
    }

    /// Nothing is silently dropped: every prose section of every record
    /// survives into the stored content, and every yaml key was understood.
    #[test]
    fn ingest_is_lossless() {
        let (ledger, report) = ingested();
        for record in parsed() {
            let claim = report.content_claim(&record.id).expect("ingested");
            let content = ledger.record(claim).unwrap().content();

            // The Forces section is deliberately decomposed rather than
            // copied, so the property there is that every force is a slice of
            // the original paragraph — the machine may split, never invent.
            if let Content::Claim(ClaimContent::Pattern { forces, .. }) = content {
                let original = record.section("Forces").expect("pattern implies Forces");
                for force in forces {
                    assert!(
                        original.contains(force.as_str()),
                        "{}: force {force:?} is not a slice of its own paragraph",
                        record.id
                    );
                }
                let recovered: usize = forces.iter().map(|f| f.chars().count()).sum();
                assert!(
                    recovered * 100 / original.chars().count().max(1) > 90,
                    "{}: the split dropped more than a tenth of the forces text",
                    record.id
                );
            }

            let carried = match content {
                Content::Claim(ClaimContent::Pattern { solution, .. }) => solution.clone(),
                Content::Claim(ClaimContent::Text { body, .. }) => body.clone(),
                Content::Hypothesis(h) => {
                    format!("{} {}", h.statement, h.falsifier.clone().unwrap_or_default())
                }
                other => panic!("{} ingested as {other:?}", record.id),
            };
            for (label, body) in &record.sections {
                if label == "Forces" {
                    continue;
                }
                // A distinctive slice, not exact equality: assembly reflows
                // the document's hard-wrapped lines.
                let probe: String = body.chars().take(48).collect();
                assert!(
                    carried.contains(probe.trim()),
                    "{} lost its {label} section",
                    record.id
                );
            }
        }
    }

    fn ingested_corpus() -> (MemoryLedger, IngestReport) {
        let mut ledger = MemoryLedger::new();
        let report = ingest_corpus(&mut ledger, &repo_root()).expect("both documents ingest");
        (ledger, report)
    }

    fn register_rows() -> Vec<crate::register::ParsedUnknown> {
        let text = std::fs::read_to_string(repo_root().join("docs/REGISTER.md")).unwrap();
        crate::register::parse_register(&text).unwrap()
    }

    /// The whole point: the register's open questions become retrievable gaps,
    /// so the engine can say "that is a registered open question" instead of
    /// "nothing found".
    #[test]
    fn open_unknowns_become_registered_gaps() {
        let (ledger, report) = ingested_corpus();
        let rows = register_rows();
        assert!(rows.len() >= 20, "the register was found, not a fragment");
        assert_eq!(report.gaps.len(), rows.len());

        let open = rows.iter().filter(|u| u.resolved.is_none()).count();
        assert!(open > 0);
        assert_eq!(ledger.registered_gaps().len(), open);

        // A decisions-only ingest has none of this — the contrast is the point.
        let mut bare = MemoryLedger::new();
        ingest_decisions(&mut bare, &repo_root()).unwrap();
        assert!(bare.registered_gaps().is_empty());
    }

    /// Resolved unknowns are answered by the very claims that settled them,
    /// and the engine refuses unless those claims are genuinely promoted.
    #[test]
    fn resolved_unknowns_are_answered_by_their_deciding_claims() {
        let (ledger, report) = ingested_corpus();
        let resolved: Vec<_> =
            register_rows().into_iter().filter(|u| u.resolved.is_some()).collect();
        assert!(resolved.len() >= 3, "U-1, U-2 and U-10 are resolved");

        for row in &resolved {
            let gap = report.gap(&row.id).expect("gap exists");
            let state = ledger.state_of(gap).expect("gap has state");
            assert_ne!(
                state,
                RecordState::Gap(tacit_core::GapState::Registered),
                "{} is resolved in the register",
                row.id
            );
        }

        // Each answer names a promoted decision claim.
        for (unknown, decision) in &report.answered {
            let claim = report.content_claim(decision).expect("decision ingested");
            assert_eq!(
                ledger.state_of(claim),
                Some(RecordState::Claim(ClaimState::Promoted)),
                "{unknown} is answered by {decision}, which must be promoted"
            );
        }
        assert!(report.answered.iter().any(|(u, d)| u == "U-1" && d == "D-0012"));
        assert!(report.answered.iter().any(|(u, d)| u == "U-2" && d == "D-0015"));
    }

    #[test]
    fn gaps_carry_their_triggers_and_territory() {
        let (ledger, report) = ingested_corpus();
        for row in register_rows().iter().filter(|u| u.resolved.is_none()) {
            let gap = report.gap(&row.id).expect("gap exists");
            let record = ledger.record(gap).expect("record");
            assert!(
                record.envelope().review_trigger().is_some(),
                "{} carries the trigger that forces it",
                row.id
            );
            let Content::Gap(content) = record.content() else { panic!("{} is a gap", row.id) };
            assert!(
                content.territory.contains(&report.unknown(&row.id).unwrap()),
                "{} covers its own anchor",
                row.id
            );
        }
    }

    /// Cross-references now resolve in both directions: a decision naming U-1
    /// and an unknown naming D-0012 both find an anchor.
    #[test]
    fn decisions_and_unknowns_cross_link() {
        let (ledger, report) = ingested_corpus();
        let projection = Projection::rebuild(&ledger);
        let view = projection
            .view(&ledger, ViewSpec::now().with_states(StateFilter::PromotedAndProposed));

        let u1 = report.unknown("U-1").expect("U-1 anchor");
        let inbound: Vec<_> = view
            .node(u1)
            .unwrap()
            .in_edges()
            .iter()
            .map(|e| ledger.entity(e.subject()).unwrap().label().to_string())
            .collect();
        assert!(
            inbound.iter().any(|l| l == "D-0006"),
            "D-0006 names U-1 in its assertion; got {inbound:?}"
        );

        // The register's gap for U-1 reaches D-0012 through its territory.
        let gap = report.gap("U-1").expect("gap");
        let Content::Gap(content) = ledger.record(gap).unwrap().content() else { panic!() };
        assert!(content.territory.contains(&report.decision("D-0012").unwrap()));
    }

    #[test]
    fn the_corpus_does_not_contradict_itself() {
        let (ledger, _) = ingested();
        assert!(ledger.contradictions().is_empty());
        assert!(Projection::rebuild(&ledger).view(&ledger, ViewSpec::now()).conflicts().is_empty());
    }

    #[test]
    fn every_promoted_content_claim_carries_a_review_trigger() {
        let (ledger, report) = ingested();
        for (id, claim) in &report.content_claims {
            let record = ledger.record(*claim).unwrap();
            assert!(
                record.envelope().review_trigger().is_some(),
                "{id} carries a review trigger"
            );
        }

        let queue = ledger.review_queue(jiff::Timestamp::now());
        // The only promoted claims without a trigger are the title
        // transcriptions, which carry no editorial commitment of their own.
        let promoted_titles = report
            .title_claims
            .iter()
            .filter(|(_, r)| {
                ledger.state_of(*r) == Some(RecordState::Claim(ClaimState::Promoted))
            })
            .count();
        assert_eq!(queue.missing_trigger.len(), promoted_titles);
        assert!(queue.due.is_empty(), "every trigger is an event, not a date");
    }
}

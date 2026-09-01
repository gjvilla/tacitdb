//! The designed "remove this" (U-11, D-0047): a rewrite that honors both the
//! law asking for removal and the ledger's refusal to lie about history.
//!
//! Append-only and erasure pull in opposite directions, and this module is
//! the agreed meeting point. The *declaration* is an ordinary appended record
//! — [`RedactionContent`], human-only, target checked, reason required — so
//! the fact that something was removed, by whom, and on what ground is as
//! permanent as any verdict. The *removal* is this rewrite: a new log where
//! the target's withheld fields are replaced by [`REDACTED`] and the event
//! carries a [`RedactionMark`] naming the declaration and fingerprinting
//! what stood there. The new log must replay through the full grammar before
//! it is allowed to take the old one's place, and a mark that names no
//! declaration refuses to load — so this door opens for lawful removal and
//! not for tampering, which is the same distinction D-0038 drew when this
//! project rewrote its own git history: record first, rewrite second,
//! witness kept.
//!
//! What this deliberately does not promise: the old bytes' afterlife. A
//! rename does not scrub disk sectors, backups, or upstream copies —
//! destroying those is the operator's legal duty and crypto-shredding is the
//! registered shape of doing it mechanically (see U-11's residuals). And the
//! fingerprint is a 64-bit hash: enough to match a retained original against
//! the husk, not a cryptographic proof of it.

use crate::content::{ClaimContent, Content, REDACTED, RedactionScope, RedactionTarget};
use crate::envelope::{Author, Evidence, RedactionMark, SourceRef};
use crate::id::EntityId;
use crate::error::Error;
use crate::id::RecordId;
use crate::journal::Event;
use crate::ledger::Ledger;
use crate::value::Value;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

/// What a rewrite did, so "did anything change" is an answer and not a diff.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RedactReport {
    /// Redaction declarations found in the log.
    pub declared: usize,
    /// Events rewritten by this run.
    pub rewritten: usize,
    /// Declarations whose work was already done by an earlier run.
    pub already_applied: usize,
}

/// Apply every declared redaction to the store at `path`.
///
/// Reads the log, husks each declared target, writes the result beside the
/// original, proves the rewritten log replays through the same validation an
/// append runs, and only then renames it into place. A store with no pending
/// declarations is rewritten into itself and reports zero.
pub fn redact_store(path: impl AsRef<Path>) -> Result<RedactReport, Error> {
    let path = path.as_ref();
    let storage = |detail: String| Error::Storage { path: path.into(), detail };
    let (mut events, _journal, _recovery) = crate::journal::read(path)?;

    // Every declaration, in log order — later scopes widen earlier ones.
    let mut orders: BTreeMap<RecordId, Vec<(RecordId, RedactionScope)>> = BTreeMap::new();
    let mut entity_orders: BTreeMap<EntityId, RecordId> = BTreeMap::new();
    let mut report = RedactReport::default();
    for event in &events {
        if let Event::Record { id, content: Content::Redaction(r), .. } = event {
            report.declared += 1;
            match r.target {
                RedactionTarget::Record(record) => {
                    orders.entry(record).or_default().push((*id, r.scope));
                }
                // An entity has one redactable part, so the latest declaration
                // is the receipt and there are no scopes to widen.
                RedactionTarget::Entity(entity) => {
                    entity_orders.insert(entity, *id);
                }
            }
        }
    }

    for event in &mut events {
        let (id, mark) = match &*event {
            Event::Entity { id, redacted, .. } => (*id, redacted.clone()),
            _ => continue,
        };
        let Some(order) = entity_orders.get(&id) else { continue };
        if mark.is_some_and(|mark| mark.by == *order) {
            report.already_applied += 1;
            continue;
        }
        let fingerprint = fingerprint_of(event_line(&*event, path)?.as_bytes());
        if let Event::Entity { label, redacted, .. } = event {
            *label = REDACTED.into();
            *redacted = Some(RedactionMark { by: *order, fingerprint });
            report.rewritten += 1;
        }
    }

    for event in &mut events {
        let (id, mark) = match &*event {
            Event::Record { id, redacted, .. } => (*id, redacted.clone()),
            _ => continue,
        };
        let Some(pending) = orders.get(&id) else { continue };
        // Idempotence: a husk already stamped by the latest declaration on it
        // has nothing left to give up.
        let latest = pending.last().expect("orders are never empty").0;
        if mark.is_some_and(|mark| mark.by == latest) {
            report.already_applied += pending.len();
            continue;
        }
        // The fingerprint commits to the event exactly as the log held it,
        // taken before anything is touched.
        let fingerprint = fingerprint_of(event_line(&*event, path)?.as_bytes());
        if let Event::Record { author, source, evidence, redacted, content, .. } = event {
            for (_, scope) in pending {
                if matches!(scope, RedactionScope::Author | RedactionScope::Record) {
                    withhold_author(author);
                }
                if matches!(scope, RedactionScope::Content | RedactionScope::Record) {
                    withhold_content(content, evidence);
                }
                if matches!(scope, RedactionScope::Source | RedactionScope::Record) {
                    withhold_source(source);
                }
            }
            *redacted = Some(RedactionMark { by: latest, fingerprint });
            report.rewritten += 1;
        }
    }

    if report.rewritten == 0 {
        return Ok(report);
    }

    // Write beside, prove, then replace. The rewritten log is opened through
    // the ordinary load path — full replay, mark receipts checked — before it
    // may stand where the old one stood; a rewrite this module cannot load is
    // a bug here, not a store to leave behind.
    let staging = path.with_extension("redacting");
    {
        let mut file = std::fs::File::create(&staging).map_err(|e| storage(e.to_string()))?;
        for event in &events {
            let mut line =
                serde_json::to_vec(event).map_err(|e| storage(format!("encoding event: {e}")))?;
            line.push(b'\n');
            file.write_all(&line).map_err(|e| storage(e.to_string()))?;
        }
        file.sync_all().map_err(|e| storage(e.to_string()))?;
    }
    match Ledger::open(&staging) {
        Ok(opened) => drop(opened),
        Err(error) => {
            let _ = std::fs::remove_file(&staging);
            return Err(error);
        }
    }
    std::fs::rename(&staging, path).map_err(|e| storage(e.to_string()))?;
    Ok(report)
}

/// The serialized form a fingerprint commits to — the event exactly as the
/// log held it before this rewrite touched it.
fn event_line(event: &Event, path: &Path) -> Result<String, Error> {
    serde_json::to_string(event)
        .map_err(|e| Error::Storage { path: path.into(), detail: format!("encoding event: {e}") })
}

/// A 64-bit identifier of what was removed, hex so the log stays greppable.
fn fingerprint_of(bytes: &[u8]) -> String {
    // FNV-1a: dependency-free and stable across runs, which is all an
    // identifier needs. Not collision-resistant against an adversary — the
    // limit is stated where the mark is defined.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn withhold_author(author: &mut Author) {
    author.name = REDACTED.into();
    author.detail = None;
}

/// The reference goes, the channel stays: a URL can carry a person, a kind
/// of provenance cannot.
fn withhold_source(source: &mut SourceRef) {
    source.reference = None;
}

/// Replace prose, keep structure. Entity references, verdict actions, and
/// timestamps are what replay stands on; every field a person's words or
/// details could hide in is emptied or marked.
fn withhold_content(content: &mut Content, evidence: &mut [Evidence]) {
    for item in evidence.iter_mut() {
        item.span = None;
    }
    match content {
        Content::Claim(claim) => match claim {
            ClaimContent::Attribute { value, .. } => *value = Value::Text(REDACTED.into()),
            ClaimContent::Relation { properties, .. } => properties.clear(),
            ClaimContent::Pattern { context, forces, solution, .. } => {
                *context = REDACTED.into();
                forces.clear();
                *solution = REDACTED.into();
            }
            ClaimContent::Text { body, .. } => *body = REDACTED.into(),
        },
        Content::Gap(gap) => gap.question = REDACTED.into(),
        Content::Hypothesis(h) => {
            h.statement = REDACTED.into();
            h.falsifier = None;
        }
        Content::Verdict(v) => v.rationale = None,
        // A redaction's reason can itself carry a person's details, so a
        // redaction can be redacted. The marker satisfies the non-empty rule:
        // the ground is withheld, not unstated.
        Content::Redaction(r) => r.reason = REDACTED.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{RedactionContent, VerdictAction, VerdictContent};
    use crate::envelope::SourceRef;
    use crate::record::Draft;
    use crate::state::{ClaimState, RecordState};
    use std::path::PathBuf;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let mut path = std::env::temp_dir();
            path.push(format!("tacit-redact-{name}-{}.log", std::process::id()));
            let _ = std::fs::remove_file(&path);
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn draft(author: Author, content: Content) -> Draft {
        Draft::new(author, SourceRef::channel("interview"), content)
    }

    fn author() -> Author {
        Author {
            name: "A Real Person".into(),
            kind: crate::envelope::AuthorKind::Human,
            detail: Some("a.real.person@example.invalid".into()),
        }
    }

    /// A store holding one promoted claim by a named person, plus the ids.
    fn store(path: &Path) -> (Ledger, RecordId) {
        let mut ledger = Ledger::open(path).unwrap().ledger;
        let subject = ledger.add_entity("topic", "torque").unwrap();
        let claim = ledger
            .append(draft(
                author(),
                Content::Claim(ClaimContent::Text {
                    body: "the fastener seats at twenty four newton metres".into(),
                    about: vec![subject],
                }),
            ))
            .unwrap();
        ledger
            .append(draft(
                Author::human("Reviewer"),
                Content::Verdict(VerdictContent {
                    action: VerdictAction::Promote { target: claim, retiring: None },
                    rationale: Some("agreed in review".into()),
                }),
            ))
            .unwrap();
        (ledger, claim)
    }

    fn order(ledger: &mut Ledger, target: RecordId, scope: RedactionScope) -> RecordId {
        ledger
            .append(draft(
                Author::human("Keeper"),
                Content::Redaction(RedactionContent {
                    target: RedactionTarget::Record(target),
                    scope,
                    reason: "erasure request under the applicable law".into(),
                }),
            ))
            .unwrap()
    }

    /// Removal is a person's act, like a verdict and for the same reason.
    #[test]
    fn an_agent_cannot_order_a_redaction() {
        let scratch = Scratch::new("agent");
        let (mut ledger, claim) = store(scratch.path());
        let refused = ledger.append(draft(
            Author::agent("assistant"),
            Content::Redaction(RedactionContent {
                target: RedactionTarget::Record(claim),
                scope: RedactionScope::Record,
                reason: "asked nicely".into(),
            }),
        ));
        assert!(matches!(refused, Err(Error::VerdictRequiresHumanAuthor)));
    }

    /// A removal with no stated ground is indistinguishable from tampering.
    #[test]
    fn a_redaction_states_its_ground_and_its_target_exists() {
        let scratch = Scratch::new("ground");
        let (mut ledger, claim) = store(scratch.path());
        let unreasoned = ledger.append(draft(
            Author::human("Keeper"),
            Content::Redaction(RedactionContent {
                target: RedactionTarget::Record(claim),
                scope: RedactionScope::Record,
                reason: "  ".into(),
            }),
        ));
        assert!(matches!(unreasoned, Err(Error::EmptyRedactionReason)));

        let nothing = ledger.append(draft(
            Author::human("Keeper"),
            Content::Redaction(RedactionContent {
                target: RedactionTarget::Record(RecordId::mint()),
                scope: RedactionScope::Record,
                reason: "aimed at nothing".into(),
            }),
        ));
        assert!(matches!(nothing, Err(Error::UnknownRecord(_))));
    }

    /// The end-to-end shape: declare, rewrite, reopen. The name is gone, the
    /// receipt is present, and the record's state never moved — a promoted
    /// claim is still promoted after its author is withheld.
    #[test]
    fn a_redacted_author_is_withheld_and_the_state_survives() {
        let scratch = Scratch::new("author");
        let (mut ledger, claim) = store(scratch.path());
        let declaration = order(&mut ledger, claim, RedactionScope::Author);
        drop(ledger);

        let report = redact_store(scratch.path()).unwrap();
        assert_eq!(report, RedactReport { declared: 1, rewritten: 1, already_applied: 0 });

        let reopened = Ledger::open(scratch.path()).unwrap().ledger;
        let record = reopened.record(claim).unwrap();
        assert_eq!(record.envelope().author().name, REDACTED);
        assert_eq!(record.envelope().author().detail, None);
        let mark = record.envelope().redacted().expect("the receipt is on the husk");
        assert_eq!(mark.by, declaration);
        assert_eq!(mark.fingerprint.len(), 16);
        assert_eq!(
            reopened.state_of(claim),
            Some(RecordState::Claim(ClaimState::Promoted)),
            "withholding a name moves no state"
        );
        // The body was not in scope and is untouched.
        assert!(matches!(
            record.content(),
            Content::Claim(ClaimContent::Text { body, .. }) if body.contains("fastener")
        ));
        // And the raw bytes really are gone.
        let text = std::fs::read_to_string(scratch.path()).unwrap();
        assert!(!text.contains("A Real Person"));
        assert!(!text.contains("example.invalid"));
    }

    /// Content scope: the prose is withheld, the structure and the verdicts
    /// that fold this record's state replay untouched, and a rebuilt index no
    /// longer holds the words.
    #[test]
    fn redacted_content_leaves_the_index_and_keeps_the_chain() {
        let scratch = Scratch::new("content");
        let (mut ledger, claim) = store(scratch.path());
        order(&mut ledger, claim, RedactionScope::Content);
        drop(ledger);
        redact_store(scratch.path()).unwrap();

        let reopened = Ledger::open(scratch.path()).unwrap().ledger;
        assert!(matches!(
            reopened.record(claim).unwrap().content(),
            Content::Claim(ClaimContent::Text { body, .. }) if body == REDACTED
        ));
        assert_eq!(reopened.state_of(claim), Some(RecordState::Claim(ClaimState::Promoted)));
        let index = crate::retrieval::TextIndex::rebuild(&reopened);
        assert_eq!(index.postings_len("fastener"), None, "the words left the index");
    }

    /// Running the rewrite again finds its work done and touches nothing.
    #[test]
    fn a_second_rewrite_is_a_no_op() {
        let scratch = Scratch::new("idempotent");
        let (mut ledger, claim) = store(scratch.path());
        order(&mut ledger, claim, RedactionScope::Record);
        drop(ledger);
        redact_store(scratch.path()).unwrap();
        let before = std::fs::read_to_string(scratch.path()).unwrap();
        let again = redact_store(scratch.path()).unwrap();
        assert_eq!(again, RedactReport { declared: 1, rewritten: 0, already_applied: 1 });
        assert_eq!(before, std::fs::read_to_string(scratch.path()).unwrap());
    }

    /// The receipt check: "redacted" is not a word anyone may write over
    /// anything. A mark naming a record that is not a redaction of the husk
    /// refuses to load — which closes the door the mark would otherwise open,
    /// since the rewrite path is the load path's only source of husks.
    #[test]
    fn a_forged_mark_refuses_to_load() {
        let scratch = Scratch::new("forged");
        let (ledger, claim) = store(scratch.path());
        drop(ledger);
        // Hand-edit the log: stamp the claim as redacted "by" the verdict.
        let text = std::fs::read_to_string(scratch.path()).unwrap();
        let verdict_id = text
            .lines()
            .find(|l| l.contains("Promote"))
            .and_then(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .and_then(|v| v.get("id").and_then(|i| i.as_str().map(String::from)))
            .expect("the verdict line has an id");
        let forged: String = text
            .lines()
            .map(|line| {
                if line.contains("fastener") {
                    line.replacen(
                        "\"content\":",
                        &format!(
                            "\"redacted\":{{\"by\":\"{verdict_id}\",\"fingerprint\":\"0000000000000000\"}},\"content\":"
                        ),
                        1,
                    )
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        std::fs::write(scratch.path(), forged).unwrap();

        let refused = Ledger::open(scratch.path());
        assert!(
            matches!(refused, Err(Error::UnattestedRedaction { record, .. }) if record == claim),
            "a husk without its receipt is a forgery"
        );
    }

    /// U-46's first gap, closed and held down: a person modeled as an entity
    /// loses their name from the label, the receipt sits on the entity, every
    /// record about the entity still resolves by id, and the bytes are gone.
    #[test]
    fn an_entity_label_is_withheld_and_the_graph_survives() {
        let scratch = Scratch::new("entity");
        let mut ledger = Ledger::open(scratch.path()).unwrap().ledger;
        let person = ledger.add_entity("person", "Firstname Lastname").unwrap();
        let claim = ledger
            .append(draft(
                Author::human("Keeper"),
                Content::Claim(ClaimContent::Text {
                    body: "chaired the working group through both revisions".into(),
                    about: vec![person],
                }),
            ))
            .unwrap();
        let declaration = ledger
            .append(draft(
                Author::human("Keeper"),
                Content::Redaction(RedactionContent {
                    target: RedactionTarget::Entity(person),
                    scope: RedactionScope::Record,
                    reason: "erasure request under the applicable law".into(),
                }),
            ))
            .unwrap();
        drop(ledger);

        let report = redact_store(scratch.path()).unwrap();
        assert_eq!(report.rewritten, 1);

        let reopened = Ledger::open(scratch.path()).unwrap().ledger;
        let entity = reopened.entity(person).unwrap();
        assert_eq!(entity.label(), REDACTED);
        assert_eq!(entity.kind(), "person", "kind is structure and survives");
        assert_eq!(entity.redacted().unwrap().by, declaration);
        // The record about the entity is untouched and still resolves.
        assert!(matches!(
            reopened.record(claim).unwrap().content(),
            Content::Claim(ClaimContent::Text { about, .. }) if about == &vec![person]
        ));
        let text = std::fs::read_to_string(scratch.path()).unwrap();
        assert!(!text.contains("Lastname"), "the name left the bytes");
    }

    /// The receipt rule, worn by an entity: a hand-stamped label husk that
    /// names no declaration refuses to load.
    #[test]
    fn a_forged_entity_mark_refuses_to_load() {
        let scratch = Scratch::new("entity-forged");
        let (ledger, _claim) = store(scratch.path());
        drop(ledger);
        let text = std::fs::read_to_string(scratch.path()).unwrap();
        let forged: String = text
            .lines()
            .map(|line| {
                if line.contains("\"entity\"") && line.contains("torque") {
                    line.replacen(
                        "\"label\":",
                        "\"redacted\":{\"by\":\"01ARZ3NDEKTSV4RRFFQ69G5FAV\",\"fingerprint\":\"0000000000000000\"},\"label\":",
                        1,
                    )
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        std::fs::write(scratch.path(), forged).unwrap();
        assert!(matches!(
            Ledger::open(scratch.path()),
            Err(Error::UnattestedEntityRedaction { .. })
        ));
    }

    /// U-46's second gap: a source reference can carry a person as surely as
    /// a body can. The reference goes, the channel stays.
    #[test]
    fn a_source_reference_is_withheld_under_source_scope() {
        let scratch = Scratch::new("source");
        let mut ledger = Ledger::open(scratch.path()).unwrap().ledger;
        let subject = ledger.add_entity("topic", "torque").unwrap();
        let claim = ledger
            .append(Draft::new(
                author(),
                SourceRef {
                    channel: "interview".into(),
                    reference: Some("notes taken by A Real Person, 2026-08-01".into()),
                },
                Content::Claim(ClaimContent::Text {
                    body: "the fastener seats at twenty four newton metres".into(),
                    about: vec![subject],
                }),
            ))
            .unwrap();
        order(&mut ledger, claim, RedactionScope::Source);
        drop(ledger);
        redact_store(scratch.path()).unwrap();

        let reopened = Ledger::open(scratch.path()).unwrap().ledger;
        let envelope = reopened.record(claim).unwrap().envelope();
        assert_eq!(envelope.source().channel, "interview", "a kind of provenance stays");
        assert_eq!(envelope.source().reference, None);
        // Source scope touches nothing else.
        assert_eq!(envelope.author().name, "A Real Person");
        let text = std::fs::read_to_string(scratch.path()).unwrap();
        assert!(!text.contains("notes taken by"));
    }

    /// Redacting the author of a verdict keeps replay legal: the name is
    /// withheld and the fact that a human declared it survives, which is what
    /// invariant 5 actually needs.
    #[test]
    fn a_verdicts_author_can_be_withheld_without_breaking_replay() {
        let scratch = Scratch::new("verdict");
        let (mut ledger, claim) = store(scratch.path());
        let verdict = ledger.history(claim)[0].id();
        order(&mut ledger, verdict, RedactionScope::Record);
        drop(ledger);
        redact_store(scratch.path()).unwrap();

        let reopened = Ledger::open(scratch.path()).unwrap().ledger;
        let husk = reopened.record(verdict).unwrap();
        assert_eq!(husk.envelope().author().name, REDACTED);
        assert!(matches!(
            husk.content(),
            Content::Verdict(VerdictContent { rationale: None, .. })
        ));
        assert_eq!(
            reopened.state_of(claim),
            Some(RecordState::Claim(ClaimState::Promoted)),
            "the ruling stands though the ruler's name is withheld"
        );
    }
}

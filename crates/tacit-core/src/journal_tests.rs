//! Durability tests. The load path is the one that matters: a store is only
//! as trustworthy as what it refuses to load.

use crate::content::{ClaimContent, Content, GapContent, VerdictAction, VerdictContent};
use crate::envelope::{Author, SourceRef};
use crate::id::{EntityId, RecordId};
use crate::ledger::Ledger;
use crate::measurement::MeasurementTarget;
use crate::record::Draft;
use crate::state::{ClaimState, RecordState};
use crate::value::Value;
use jiff::Timestamp;
use std::path::PathBuf;

/// A scratch path that cleans itself up.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let mut path = std::env::temp_dir();
        path.push(format!("tacit-{name}-{}.log", std::process::id()));
        let _ = std::fs::remove_file(&path);
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }

    fn text(&self) -> String {
        std::fs::read_to_string(&self.0).unwrap_or_default()
    }

    fn write(&self, text: &str) {
        std::fs::write(&self.0, text).expect("write scratch");
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn claim(subject: EntityId, body: &str, author: Author) -> Draft {
    Draft::new(
        author,
        SourceRef::channel("interview"),
        Content::Claim(ClaimContent::Text { body: body.into(), about: vec![subject] }),
    )
}

fn promote(target: RecordId) -> Draft {
    Draft::new(
        Author::human("Greg"),
        SourceRef::channel("huddle"),
        Content::Verdict(VerdictContent {
            action: VerdictAction::Promote { target, retiring: None },
            rationale: Some("agreed at the huddle".into()),
        }),
    )
}

/// Build a small but representative ledger: entities, prose, an attribute, a
/// promotion, a gap, and a measurement.
fn seed(ledger: &mut Ledger) -> (EntityId, RecordId) {
    let subject = ledger.add_entity("process", "torque check").unwrap();
    let source = ledger.add_source("docs/NOTES.md").unwrap();
    let mut with_evidence = claim(subject, "the fastener seats at twenty four newton metres", Author::human("Maria"));
    with_evidence.evidence.push(crate::envelope::Evidence { source, span: Some("p. 3".into()) });
    let promoted = ledger.append(with_evidence).unwrap();
    ledger.append(promote(promoted)).unwrap();

    ledger
        .append(Draft::new(
            Author::agent("miner"),
            SourceRef::channel("pipeline"),
            Content::Claim(ClaimContent::Attribute {
                subject,
                name: "spec_nm".into(),
                value: Value::Number(24.0),
            }),
        ))
        .unwrap();
    ledger
        .append(Draft::new(
            Author::agent("assistant"),
            SourceRef::channel("chat"),
            Content::Gap(GapContent {
                question: "what torque in cold weather?".into(),
                territory: vec![subject],
            }),
        ))
        .unwrap();
    ledger
        .record_measurement(
            MeasurementTarget::Entity(subject),
            "observations",
            7.0,
            Author::agent("counter"),
            Timestamp::now(),
        )
        .unwrap();
    (subject, promoted)
}

#[test]
fn a_ledger_survives_a_round_trip() {
    let scratch = Scratch::new("roundtrip");
    let (subject, promoted, log_len) = {
        let mut opened = Ledger::open(scratch.path()).unwrap();
        assert_eq!(opened.recovery.events_replayed, 0, "a new store is empty");
        let (subject, promoted) = seed(&mut opened.ledger);
        (subject, promoted, opened.ledger.log().len())
    };

    let reopened = Ledger::open(scratch.path()).unwrap();
    let ledger = &reopened.ledger;
    assert_eq!(ledger.log().len(), log_len);
    assert_eq!(ledger.entities().count(), 2);
    assert_eq!(
        ledger.state_of(promoted),
        Some(RecordState::Claim(ClaimState::Promoted)),
        "state survives because the verdict was replayed, not because it was stored"
    );
    assert_eq!(ledger.registered_gaps().len(), 1);
    assert_eq!(ledger.pending_proposals().len(), 1);

    // Envelope detail survives intact.
    let record = ledger.record(promoted).unwrap();
    assert_eq!(record.envelope().author().name, "Maria");
    assert_eq!(record.envelope().evidence().len(), 1);
    assert_eq!(record.envelope().evidence()[0].span.as_deref(), Some("p. 3"));

    // And so does the instrument panel.
    let measurement = ledger
        .measurement(MeasurementTarget::Entity(subject), "observations")
        .expect("measurement survives");
    assert_eq!(measurement.value, 7.0);
}

#[test]
fn record_time_travel_survives_a_reload() {
    let scratch = Scratch::new("timetravel");
    let (promoted, before) = {
        let mut opened = Ledger::open(scratch.path()).unwrap();
        let subject = opened.ledger.add_entity("process", "p").unwrap();
        let claim = opened.ledger.append(claim(subject, "a claim", Author::human("G"))).unwrap();
        let before = Timestamp::now();
        opened.ledger.append(promote(claim)).unwrap();
        (claim, before)
    };
    let reopened = Ledger::open(scratch.path()).unwrap();
    assert_eq!(
        reopened.ledger.state_of_at(promoted, before),
        Some(RecordState::Claim(ClaimState::Proposed)),
        "the past is reconstructed from the replayed verdicts"
    );
    assert_eq!(
        reopened.ledger.state_of(promoted),
        Some(RecordState::Claim(ClaimState::Promoted))
    );
}

/// The property the whole design turns on: a store is re-validated, not
/// trusted. Promotion is not a field anyone can write.
#[test]
fn a_hand_edited_promotion_cannot_load() {
    let scratch = Scratch::new("forged");
    {
        let mut opened = Ledger::open(scratch.path()).unwrap();
        let subject = opened.ledger.add_entity("process", "p").unwrap();
        opened.ledger.append(claim(subject, "an unpromoted claim", Author::human("G"))).unwrap();
    }

    // Forge a promote verdict authored by an agent — exactly what invariant 6
    // forbids, written straight into the log.
    let text = scratch.text();
    let claim_id = text
        .lines()
        .find(|l| l.contains("\"Claim\""))
        .and_then(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .and_then(|v| v["id"].as_str().map(str::to_string))
        .expect("a claim id");
    let forged = serde_json::json!({
        "event": "record",
        "id": RecordId::mint().to_string().replace("rec_", ""),
        "recorded_at": Timestamp::now(),
        "envelope_version": 1,
        "author": {"name": "sneaky", "kind": "Agent", "detail": null},
        "source": {"channel": "forged", "reference": null},
        "valid_from": Timestamp::now(),
        "content": {"Verdict": {"action": {"Promote": {"target": claim_id, "retiring": null}}, "rationale": null}}
    });
    scratch.write(&format!("{text}{forged}\n"));

    let error = Ledger::open(scratch.path()).expect_err("the forgery must not load");
    assert!(
        matches!(error, crate::error::Error::VerdictRequiresHumanAuthor),
        "expected the grammar to reject it, got {error}"
    );
}

/// A verdict whose target does not exist, or whose transition is illegal, is
/// rejected on load for the same reason it would be rejected live.
#[test]
fn an_illegal_transition_cannot_load() {
    let scratch = Scratch::new("illegal");
    let claim_line = {
        let mut opened = Ledger::open(scratch.path()).unwrap();
        let subject = opened.ledger.add_entity("process", "p").unwrap();
        let id = opened.ledger.append(claim(subject, "a claim", Author::human("G"))).unwrap();
        opened.ledger.append(promote(id)).unwrap();
        scratch.text()
    };
    // Duplicate the promote line: promoting an already-promoted claim.
    let promote_line = claim_line
        .lines()
        .find(|l| l.contains("Promote"))
        .expect("a promote line")
        .to_string();
    scratch.write(&format!("{claim_line}{promote_line}\n"));

    let error = Ledger::open(scratch.path()).expect_err("a double promotion must not load");
    assert!(
        matches!(error, crate::error::Error::IllegalTransition { .. }),
        "got {error}"
    );
}

/// A crash between write and sync leaves a partial line no reader ever saw.
/// Dropping it restores the last consistent state; anything malformed earlier
/// is corruption and refuses.
#[test]
fn a_torn_final_line_is_recovered_and_corruption_is_not() {
    let scratch = Scratch::new("torn");
    let complete = {
        let mut opened = Ledger::open(scratch.path()).unwrap();
        let subject = opened.ledger.add_entity("process", "p").unwrap();
        opened.ledger.append(claim(subject, "a whole claim", Author::human("G"))).unwrap();
        opened.ledger.log().len()
    };

    let good = scratch.text();
    scratch.write(&format!("{good}{{\"event\":\"rec"));
    let recovered = Ledger::open(scratch.path()).expect("a torn tail is recoverable");
    assert_eq!(recovered.ledger.log().len(), complete);
    assert!(recovered.recovery.truncated_bytes > 0);
    // The torn line is gone from the file, so the next append is not preceded
    // by garbage.
    assert_eq!(scratch.text(), good);

    // The same damage in the middle is corruption.
    scratch.write(&format!("{{\"event\":\"rec\n{good}"));
    let error = Ledger::open(scratch.path()).expect_err("mid-file damage must refuse");
    assert!(matches!(error, crate::error::Error::CorruptJournal { .. }), "got {error}");
}

#[test]
fn an_unreadable_envelope_version_refuses_rather_than_guessing() {
    let scratch = Scratch::new("version");
    {
        let mut opened = Ledger::open(scratch.path()).unwrap();
        let subject = opened.ledger.add_entity("process", "p").unwrap();
        opened.ledger.append(claim(subject, "a claim", Author::human("G"))).unwrap();
    }
    let bumped = scratch.text().replace("\"envelope_version\":1", "\"envelope_version\":99");
    scratch.write(&bumped);
    let error = Ledger::open(scratch.path()).expect_err("a future envelope must not be guessed at");
    assert!(
        matches!(error, crate::error::Error::UnsupportedEnvelopeVersion { found: 99, .. }),
        "got {error}"
    );
}

#[test]
fn an_in_memory_ledger_writes_nothing() {
    let mut ledger = Ledger::new();
    assert!(ledger.journal_path().is_none());
    let subject = ledger.add_entity("process", "p").unwrap();
    ledger.append(claim(subject, "a claim", Author::human("G"))).unwrap();
    assert_eq!(ledger.log().len(), 1);
}

/// Appending after a reload continues the same log rather than starting over.
#[test]
fn writes_continue_after_a_reload() {
    let scratch = Scratch::new("continue");
    {
        let mut opened = Ledger::open(scratch.path()).unwrap();
        let subject = opened.ledger.add_entity("process", "p").unwrap();
        opened.ledger.append(claim(subject, "first", Author::human("G"))).unwrap();
    }
    {
        let mut opened = Ledger::open(scratch.path()).unwrap();
        assert_eq!(opened.recovery.events_replayed, 2, "the entity and the claim");
        let subject = opened.ledger.find_entity("process", "p").expect("entity survives");
        opened.ledger.append(claim(subject, "second", Author::human("G"))).unwrap();
    }
    let final_open = Ledger::open(scratch.path()).unwrap();
    assert_eq!(final_open.ledger.log().len(), 2);
    assert_eq!(final_open.ledger.entities().count(), 1, "the entity is not duplicated");
}

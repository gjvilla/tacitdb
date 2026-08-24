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
use jiff::{SignedDuration, Timestamp};
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
    assert_eq!(ledger.pending_proposals().queued.len(), 1);

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
    // Explicit record-times, not the wall clock: `state_of_at(t)` includes
    // verdicts recorded *at* t, so a promote landing in the same tick as the
    // probe would make this flaky rather than wrong.
    let claim_at = Timestamp::from_second(1_700_000_000).unwrap();
    let before = Timestamp::from_second(1_700_000_100).unwrap();
    let promote_at = Timestamp::from_second(1_700_000_200).unwrap();
    let promoted = {
        let mut opened = Ledger::open(scratch.path()).unwrap();
        let subject = opened.ledger.add_entity("process", "p").unwrap();
        let claim =
            opened.ledger.append_at(claim(subject, "a claim", Author::human("G")), claim_at).unwrap();
        opened.ledger.append_at(promote(claim), promote_at).unwrap();
        claim
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

/// U-22 meets D-0019: a record-time held against a backwards clock is ahead of
/// the wall clock, and replay must still accept it. This is the seam where the
/// two guards could disagree — the append ceiling and the replay ceiling are
/// the same expression precisely so they cannot.
#[test]
fn a_record_time_held_against_a_backwards_clock_survives_a_reload() {
    let scratch = Scratch::new("clock-hold-reload");
    let ahead = Timestamp::now() + SignedDuration::from_secs(3);

    let held = {
        let mut ledger = Ledger::open(scratch.path()).unwrap().ledger;
        let subject = ledger.add_entity("process", "torque check").unwrap();
        ledger.force_log_ahead_of_clock(ahead);
        let id = ledger.append(claim(subject, "the torque check is a controlled step", Author::human("Greg"))).unwrap();
        assert_eq!(ledger.clock_holds(), 1);
        id
    };

    // Replay accepts a log that leads the clock, and says so rather than
    // refusing. The refusal was the real cost of the old rule: a clock set back
    // an hour did not block the next few appends, it made the whole store
    // unopenable — every record in it now reading as "in the future".
    let opened = Ledger::open(scratch.path()).unwrap();
    assert_eq!(opened.ledger.record(held).unwrap().envelope().recorded_at(), ahead);
    let leads = opened.recovery.leads_clock.expect("the log leads the clock");
    assert!(leads.as_secs() > 0 && leads.as_secs() <= 3);

    // Reads are unaffected; only minting a *new* record-time consults the clock.
    assert_eq!(
        opened.ledger.state_of(held),
        Some(RecordState::Claim(ClaimState::Proposed))
    );
}

/// The ordinary case, for contrast: a log written by a healthy clock reports
/// nothing, so `leads_clock` is a signal and not noise.
#[test]
fn a_log_written_by_a_healthy_clock_reports_no_skew() {
    let scratch = Scratch::new("clock-healthy");
    {
        let mut ledger = Ledger::open(scratch.path()).unwrap().ledger;
        let subject = ledger.add_entity("process", "torque check").unwrap();
        ledger.append(claim(subject, "the torque check is a controlled step", Author::human("Greg"))).unwrap();
    }
    let opened = Ledger::open(scratch.path()).unwrap();
    assert!(opened.recovery.leads_clock.is_none());
    assert_eq!(opened.ledger.clock_holds(), 0);
}

/// The migration U-28 forced: `Withdraw` gained a reason, and logs written
/// before it exists have none. They load as `Unstated` — the one reason a live
/// verdict may not give — because assigning them a real reason after the fact
/// would invent an account nobody gave.
#[test]
fn a_withdrawal_written_before_reasons_existed_loads_as_unstated() {
    use crate::content::{GapContent, WithdrawReason};

    let scratch = Scratch::new("reasonless-withdraw");
    let gap = {
        let mut ledger = Ledger::open(scratch.path()).unwrap().ledger;
        ledger
            .append(Draft::new(
                Author::human("Greg"),
                SourceRef::channel("register"),
                Content::Gap(GapContent { question: "whether to shard".into(), territory: vec![] }),
            ))
            .unwrap()
    };

    // The line the old code wrote: a Withdraw with no `reason` field at all.
    let text = scratch.text();
    let old_shape = serde_json::json!({
        "event": "record",
        "id": RecordId::mint().to_string().replace("rec_", ""),
        "recorded_at": Timestamp::now(),
        "envelope_version": 1,
        "author": {"name": "Greg", "kind": "Human", "detail": null},
        "source": {"channel": "register", "reference": null},
        "valid_from": Timestamp::now(),
        "content": {"Verdict": {"action": {"Withdraw": {"gap": gap.to_string().replace("rec_", "")}}, "rationale": null}}
    });
    scratch.write(&format!("{text}{old_shape}\n"));

    let ledger = Ledger::open(scratch.path()).expect("an old log still loads").ledger;
    assert_eq!(ledger.state_of(gap), Some(RecordState::Gap(crate::state::GapState::Withdrawn)));

    let recorded = match ledger.history(gap)[0].content() {
        Content::Verdict(v) => v.action.clone(),
        other => panic!("expected a verdict, got {other:?}"),
    };
    assert!(matches!(
        recorded,
        crate::content::VerdictAction::Withdraw { reason: WithdrawReason::Unstated, .. }
    ));

    // And the same verdict cannot be written today, which is what keeps
    // `Unstated` readable as "this predates reasons".
    let mut live = ledger;
    let err = live
        .append(Draft::new(
            Author::human("Greg"),
            SourceRef::channel("register"),
            Content::Verdict(crate::content::VerdictContent {
                action: crate::content::VerdictAction::Withdraw {
                    gap,
                    reason: WithdrawReason::Unstated,
                },
                rationale: None,
            }),
        ))
        .unwrap_err();
    assert!(matches!(err, crate::error::Error::UnstatedWithdrawReason), "got {err}");
}

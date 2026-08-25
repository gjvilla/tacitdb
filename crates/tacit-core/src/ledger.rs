use crate::content::{ClaimContent, Content, RecordKind, VerdictAction, WithdrawReason};
use crate::entity::Entity;
use crate::envelope::{Author, AuthorKind, Envelope};
use crate::error::Error;
use crate::id::{EntityId, RecordId};
use crate::journal::{Event, Journal, Recovery};
use crate::measurement::{Measurement, MeasurementTarget};
use crate::record::{Draft, Record};
use crate::state::{ClaimState, GapState, HypothesisState, RecordState};
use crate::envelope::ENVELOPE_VERSION;
use jiff::{SignedDuration, Timestamp};
use std::collections::BTreeMap;
use std::path::Path;

/// The kind reserved for entities that evidence may point at.
pub const SOURCE_KIND: &str = "source";

/// How far the wall clock may step backwards and still be treated as clock
/// discipline rather than a wrong clock (U-22).
///
/// NTP corrections on a running machine are milliseconds to seconds, and a
/// leap second is smeared. A backwards step of minutes or more is a different
/// event — a manual change, a restored snapshot, a dead RTC, a timezone bug —
/// and continuing to stamp records from a clock known to be wrong is worse
/// than refusing to write.
const CLOCK_HOLD_TOLERANCE: SignedDuration = SignedDuration::from_mins(5);

/// The ledger: the governed record store, the entity registry, and the
/// instrument panel.
///
/// Ordering note: the log is the definition of order. `RecordId` is a ULID and
/// is *not* monotonic within a millisecond, so nothing may range-scan on id in
/// place of walking the log.
#[derive(Debug, Default)]
pub struct Ledger {
    entities: BTreeMap<EntityId, Entity>,
    records: BTreeMap<RecordId, Record>,
    /// Total append order. State folds run in log order, not timestamp order,
    /// so same-millisecond appends stay deterministic.
    log: Vec<RecordId>,
    /// Record → its position in the log. The reverse of `log`, kept because
    /// finding a record's order by scanning is O(n) and callers want it inside
    /// sort comparators — which made a ranking over 35,000 candidates do a
    /// linear pass of a 68,000-entry log per comparison. Found by generating a
    /// corpus large enough to notice (D-0030).
    order: BTreeMap<RecordId, usize>,
    /// Record → verdicts touching it, in log order.
    by_target: BTreeMap<RecordId, Vec<RecordId>>,
    /// Record → the records that supersede it, in log order. The reverse of
    /// `Envelope::supersedes`, kept because the question a reader actually
    /// asks is "what replaced this?", and scanning for it is the wrong shape.
    replacements: BTreeMap<RecordId, Vec<RecordId>>,
    /// Highest record-time appended. The log must be a prefix of time.
    last_recorded_at: Option<Timestamp>,
    panel: BTreeMap<(MeasurementTarget, String), Measurement>,
    /// When present, every write is on disk before it is in memory.
    journal: Option<Journal>,
    /// How many appends have held record-time at `last_recorded_at` because
    /// the wall clock had stepped backwards. Not zero means the machine's
    /// clock moved and the ledger absorbed it — worth surfacing, never worth
    /// silently discarding.
    clock_holds: u64,
}

/// A ledger opened from disk, with what recovery had to do to get there.
#[derive(Debug)]
pub struct Opened {
    pub ledger: Ledger,
    pub recovery: Recovery,
}

/// The keeper's inbox, and what was folded out of it.
///
/// Both lists are present rather than one being filtered away silently: a
/// queue that quietly drops records is how a reviewer comes to believe they
/// have seen everything.
#[derive(Debug)]
pub struct Pending<'a> {
    /// Proposals awaiting a verdict, one per supersession chain — the wording
    /// its author currently stands behind.
    pub queued: Vec<&'a Record>,
    /// Proposals a later record replaced before anyone ruled on them.
    ///
    /// Still proposed, and correctly so: nobody ruled on them, and an author
    /// editing their own unreviewed draft is not a verdict (invariant 4). What
    /// changed is not their state but their claim on a reviewer's attention —
    /// reading a wording its author has already replaced is spent attention,
    /// and seeing two of them is worse than seeing neither.
    pub superseded: Vec<&'a Record>,
}

impl<'a> Pending<'a> {
    /// Everything still proposed, queued or not — the old flat reading, for a
    /// caller that genuinely wants all of it.
    pub fn all(&self) -> impl Iterator<Item = &'a Record> {
        self.queued.iter().copied().chain(self.superseded.iter().copied())
    }
}

/// The keeper's review work-queue (design/001 §8): promoted claims due for
/// review, and promoted claims with no trigger at all (drift hygiene).
#[derive(Debug)]
pub struct ReviewQueue<'a> {
    pub due: Vec<&'a Record>,
    pub missing_trigger: Vec<&'a Record>,
}

/// Two promoted attribute claims for the same (subject, attribute) with
/// overlapping valid-time (invariant 7). Surfaced, never auto-resolved.
#[derive(Debug)]
pub struct Contradiction<'a> {
    pub subject: EntityId,
    pub attribute: &'a str,
    pub a: &'a Record,
    pub b: &'a Record,
}

impl Ledger {
    pub fn new() -> Self {
        Self::default()
    }

    // ── Entities ────────────────────────────────────────────────────────────

    /// Fallible because a durable ledger puts the entity on disk before it is
    /// in memory: a store that cannot be written to must say so rather than
    /// carry on holding state its own log does not have.
    pub fn add_entity(
        &mut self,
        kind: impl Into<String>,
        label: impl Into<String>,
    ) -> Result<EntityId, Error> {
        let (kind, label) = (kind.into(), label.into());
        let id = EntityId::mint();
        if let Some(journal) = &mut self.journal {
            let event = Event::Entity { id, kind: kind.clone(), label: label.clone() };
            journal.append(&event)?;
        }
        self.entities.insert(id, Entity::new(id, kind, label));
        Ok(id)
    }

    /// Register a source — the only entity kind evidence may point at
    /// (design/001 §8). Exists so no caller has to type the magic string.
    pub fn add_source(&mut self, label: impl Into<String>) -> Result<EntityId, Error> {
        self.add_entity(SOURCE_KIND, label)
    }

    pub fn entity(&self, id: EntityId) -> Option<&Entity> {
        self.entities.get(&id)
    }

    pub fn entities(&self) -> impl Iterator<Item = &Entity> {
        self.entities.values()
    }

    /// Identity lookup by (kind, label). Dedup lives here rather than in a
    /// caller-side map so the ledger stays rebuildable from its own contents.
    /// Uniqueness of (kind, label) is keeper convention, not engine grammar,
    /// until U-12 decides; this returns the first match in id order.
    pub fn find_entity(&self, kind: &str, label: &str) -> Option<EntityId> {
        self.entities
            .values()
            .find(|e| e.kind() == kind && e.label() == label)
            .map(|e| e.id())
    }

    pub fn upsert_entity(&mut self, kind: &str, label: &str) -> Result<EntityId, Error> {
        match self.find_entity(kind, label) {
            Some(id) => Ok(id),
            None => self.add_entity(kind, label),
        }
    }

    // ── The governed ledger ─────────────────────────────────────────────────

    pub fn append(&mut self, draft: Draft) -> Result<RecordId, Error> {
        let (recorded_at, held) = self.next_record_time()?;
        let id = self.append_at(draft, recorded_at)?;
        self.clock_holds += u64::from(held);
        Ok(id)
    }

    /// The record-time this append will carry, and whether the clock had to be
    /// held to produce it.
    ///
    /// The invariant `state_of_at` depends on is monotonicity, not agreement
    /// with the wall clock, so a backwards step inside the tolerance holds the
    /// line at the last record-time rather than failing. Beyond it, the append
    /// refuses — and the error names the moment appends resume, because a
    /// guard with no stated way out is the whole of U-22's complaint.
    fn next_record_time(&self) -> Result<(Timestamp, bool), Error> {
        let now = Timestamp::now();
        let Some(last) = self.last_recorded_at else { return Ok((now, false)) };
        if now >= last {
            return Ok((now, false));
        }
        let behind = last.duration_since(now);
        if behind > CLOCK_HOLD_TOLERANCE {
            return Err(Error::ClockWentBackwards { now, last, behind });
        }
        Ok((last, true))
    }

    /// Appends that held the line against a backwards clock step.
    pub fn clock_holds(&self) -> u64 {
        self.clock_holds
    }

    /// Put the ledger in the state a machine whose clock has just stepped
    /// backwards is in: the log leads the wall clock. Test-only, because
    /// `append_at` cannot produce it — no record is ever written ahead of the
    /// clock, so the only way the log can lead is for the clock to have moved.
    #[cfg(test)]
    pub(crate) fn force_log_ahead_of_clock(&mut self, recorded_at: Timestamp) {
        self.last_recorded_at = Some(recorded_at);
    }

    /// Append with an explicit record-time. Deliberately not public: invariant
    /// 3 says record-time is engine-assigned, and a public backdating door
    /// would make `state_of_at` unfalsifiable. Tests and property tests inside
    /// the crate use it for determinism.
    pub(crate) fn append_at(
        &mut self,
        draft: Draft,
        recorded_at: Timestamp,
    ) -> Result<RecordId, Error> {
        // A record may not claim a time later than now — except to hold the
        // line at a record-time this ledger has already issued, which is how a
        // backwards clock step is absorbed without breaking monotonicity.
        //
        // This is an append-path guard and lives here rather than in
        // `validate`, because it is not a grammar rule. Replay deliberately
        // does not apply it: a log written before the clock stepped backwards
        // legitimately leads the clock, and refusing to open it would make a
        // wrong clock cost the whole store rather than the next few writes.
        // Nothing is given up — what `state_of_at` needs is monotonicity
        // within the log, and replay still enforces that.
        let now = Timestamp::now();
        let ceiling = self.last_recorded_at.map_or(now, |last| last.max(now));
        if recorded_at > ceiling {
            return Err(Error::FutureRecordTime { proposed: recorded_at, now });
        }

        // The same distinction, met a second time: `Unstated` is what a log
        // written before reasons existed *reads as*, not an account a verdict
        // may give. Illegal to claim, not illegal to hold — so it belongs on
        // the write path, and replay leaves it alone. Nothing is given up: the
        // reason confers no authority, and the author and transition checks
        // that do are still enforced on load.
        if let Content::Verdict(verdict) = &draft.content
            && matches!(
                verdict.action,
                VerdictAction::Withdraw { reason: WithdrawReason::Unstated, .. }
                    | VerdictAction::Abandon { reason: WithdrawReason::Unstated, .. }
            )
        {
            return Err(Error::UnstatedWithdrawReason);
        }

        self.validate(&draft, recorded_at)?;
        let id = RecordId::mint();
        self.write(id, draft, recorded_at, ENVELOPE_VERSION)
    }

    /// Every check an append runs, with nothing mutated. Replay calls this too,
    /// which is what makes a loaded ledger as trustworthy as a live one.
    fn validate(&self, draft: &Draft, recorded_at: Timestamp) -> Result<(), Error> {
        if let Some(last) = self.last_recorded_at
            && recorded_at < last
        {
            return Err(Error::NonMonotonicRecordTime { proposed: recorded_at, last });
        }

        if let (Some(from), Some(to)) = (draft.valid_from, draft.valid_to)
            && to <= from
        {
            return Err(Error::InvalidValidity);
        }
        if let Some(to) = draft.valid_to
            && draft.valid_from.is_none()
            && to <= recorded_at
        {
            return Err(Error::InvalidValidity);
        }

        for ev in &draft.evidence {
            let entity = self.entities.get(&ev.source).ok_or(Error::UnknownEntity(ev.source))?;
            if entity.kind() != SOURCE_KIND {
                return Err(Error::EvidenceNotSource(ev.source, entity.kind().to_string()));
            }
        }

        if let Some(prior) = draft.supersedes {
            let replaced = self.records.get(&prior).ok_or(Error::UnknownRecord(prior))?;
            // Supersession is "this record replaces that one", which only means
            // anything between records of one kind: a gap does not replace a
            // claim, and a claim does not replace a question. The link is where
            // supersession lives — a verdict may *say* superseded, and this is
            // what makes the successor findable.
            if replaced.kind() != draft.content.kind() {
                return Err(Error::SupersedesDifferentKind {
                    prior,
                    replaced: replaced.kind(),
                    proposed: draft.content.kind(),
                });
            }
        }

        match &draft.content {
            Content::Claim(claim) => {
                self.check_entities_exist(&claim.entity_refs())?;
                Self::check_no_repeats(claim.ref_list())?;
            }
            Content::Gap(gap) => {
                self.check_entities_exist(&gap.territory)?;
                Self::check_no_repeats(&gap.territory)?;
            }
            Content::Hypothesis(_) => {}
            Content::Verdict(verdict) => {
                // Invariants 5 and 6: every verdict is human-declared; agents
                // may author claims, gaps, hypotheses, and measurements only.
                if draft.author.kind != AuthorKind::Human {
                    return Err(Error::VerdictRequiresHumanAuthor);
                }
                self.check_verdict(&verdict.action)?;
            }
        }
        Ok(())
    }

    /// Journal first, then commit: a failed write leaves the ledger exactly as
    /// it was rather than ahead of its own log.
    fn write(
        &mut self,
        id: RecordId,
        draft: Draft,
        recorded_at: Timestamp,
        envelope_version: u16,
    ) -> Result<RecordId, Error> {
        if envelope_version != ENVELOPE_VERSION {
            return Err(Error::UnsupportedEnvelopeVersion {
                found: envelope_version,
                supported: ENVELOPE_VERSION,
            });
        }
        let valid_from = draft.valid_from.unwrap_or(recorded_at);

        if let Some(journal) = &mut self.journal {
            let event = Event::Record {
                id,
                recorded_at,
                envelope_version,
                author: draft.author.clone(),
                source: draft.source.clone(),
                valid_from,
                valid_to: draft.valid_to,
                evidence: draft.evidence.clone(),
                review_trigger: draft.review_trigger.clone(),
                supersedes: draft.supersedes,
                content: draft.content.clone(),
            };
            journal.append(&event)?;
        }

        let envelope = Envelope::seal(
            draft.author,
            draft.source,
            recorded_at,
            Some(valid_from),
            draft.valid_to,
            draft.evidence,
            draft.review_trigger,
            draft.supersedes,
        );
        self.commit(Record::new(id, envelope, draft.content), recorded_at);
        Ok(id)
    }

    fn commit(&mut self, record: Record, recorded_at: Timestamp) {
        let id = record.id();
        if let Content::Verdict(v) = record.content() {
            for touched in v.action.touched() {
                self.by_target.entry(touched).or_default().push(id);
            }
        }
        if let Some(prior) = record.envelope().supersedes() {
            self.replacements.entry(prior).or_default().push(id);
        }
        self.records.insert(id, record);
        self.order.insert(id, self.log.len());
        self.log.push(id);
        self.last_recorded_at = Some(recorded_at);
    }

    // ── Durability ──────────────────────────────────────────────────────────

    /// Open a ledger backed by an append-only log, replaying what is there.
    ///
    /// Replay is not deserialization: every event goes through the same
    /// validation an append runs, against the state the events before it
    /// built. A log that has been edited to promote something cannot load,
    /// because promotion is not a field — it is a fold over verdicts, and a
    /// forged verdict still has to be legal.
    pub fn open(path: impl AsRef<Path>) -> Result<Opened, Error> {
        let (events, journal, mut recovery) = crate::journal::read(path.as_ref())?;
        let mut ledger = Ledger::new();
        for event in events {
            ledger.replay(event)?;
        }
        ledger.journal = Some(journal);
        // Surfaced, not corrected: a log ahead of the clock means the machine's
        // clock moved, and a caller that silently swallowed that would be
        // hiding the one fact that explains the next refused append (U-22).
        let now = Timestamp::now();
        recovery.leads_clock = ledger
            .last_recorded_at
            .filter(|last| *last > now)
            .map(|last| last.duration_since(now));
        Ok(Opened { ledger, recovery })
    }

    fn replay(&mut self, event: Event) -> Result<(), Error> {
        match event {
            Event::Entity { id, kind, label } => {
                self.entities.insert(id, Entity::new(id, kind, label));
            }
            Event::Record {
                id,
                recorded_at,
                envelope_version,
                author,
                source,
                valid_from,
                valid_to,
                evidence,
                review_trigger,
                supersedes,
                content,
            } => {
                let draft = Draft {
                    author,
                    source,
                    valid_from: Some(valid_from),
                    valid_to,
                    evidence,
                    review_trigger,
                    supersedes,
                    content,
                };
                self.validate(&draft, recorded_at)?;
                self.write(id, draft, recorded_at, envelope_version)?;
            }
            Event::Measurement { target, name, value, at, by } => {
                self.apply_measurement(target, name, value, by, at)?;
            }
        }
        Ok(())
    }

    /// The path this ledger is durable to, if any.
    pub fn journal_path(&self) -> Option<&Path> {
        self.journal.as_ref().map(|j| j.path())
    }

    fn check_entities_exist(&self, refs: &[EntityId]) -> Result<(), Error> {
        for entity in refs {
            if !self.entities.contains_key(entity) {
                return Err(Error::UnknownEntity(*entity));
            }
        }
        Ok(())
    }

    /// A repeat in an "about" list is rejected rather than quietly folded
    /// away: it would multiply one record across every entity-scoped read, so
    /// a caller counting "open questions about A" would see one gap twice.
    fn check_no_repeats(refs: &[EntityId]) -> Result<(), Error> {
        for (i, entity) in refs.iter().enumerate() {
            if refs[..i].contains(entity) {
                return Err(Error::DuplicateEntityRef(*entity));
            }
        }
        Ok(())
    }

    fn check_verdict(&self, action: &VerdictAction) -> Result<(), Error> {
        let name = action.name();
        let expect_kind = |id: RecordId, expected: RecordKind| -> Result<&Record, Error> {
            let rec = self.records.get(&id).ok_or(Error::UnknownRecord(id))?;
            if rec.kind() != expected {
                return Err(Error::WrongTargetKind {
                    action: name,
                    target: id,
                    expected,
                    actual: rec.kind(),
                });
            }
            Ok(rec)
        };
        let expect_state =
            |id: RecordId, wanted: RecordState, state: RecordState| -> Result<(), Error> {
                if state == wanted {
                    Ok(())
                } else {
                    Err(Error::IllegalTransition { action: name, target: id, state })
                }
            };

        match action {
            VerdictAction::Promote { target, retiring } => {
                expect_kind(*target, RecordKind::Claim)?;
                expect_state(
                    *target,
                    RecordState::Claim(ClaimState::Proposed),
                    self.state_of(*target).expect("target exists"),
                )?;
                if let Some(retiring) = retiring {
                    if retiring == target {
                        return Err(Error::PromoteRetireSameRecord);
                    }
                    expect_kind(*retiring, RecordKind::Claim)?;
                    expect_state(
                        *retiring,
                        RecordState::Claim(ClaimState::Promoted),
                        self.state_of(*retiring).expect("retiring exists"),
                    )?;
                }
                Ok(())
            }
            VerdictAction::Retire { target, .. } => {
                expect_kind(*target, RecordKind::Claim)?;
                expect_state(
                    *target,
                    RecordState::Claim(ClaimState::Promoted),
                    self.state_of(*target).expect("target exists"),
                )
            }
            VerdictAction::Reject { target } => {
                expect_kind(*target, RecordKind::Claim)?;
                expect_state(
                    *target,
                    RecordState::Claim(ClaimState::Proposed),
                    self.state_of(*target).expect("target exists"),
                )
            }
            VerdictAction::Answer { gap, with_claim } => {
                expect_kind(*gap, RecordKind::Gap)?;
                expect_state(
                    *gap,
                    RecordState::Gap(GapState::Registered),
                    self.state_of(*gap).expect("gap exists"),
                )?;
                expect_kind(*with_claim, RecordKind::Claim)?;
                let state = self.state_of(*with_claim).expect("claim exists");
                if state != RecordState::Claim(ClaimState::Promoted) {
                    return Err(Error::AnswerRequiresPromotedClaim { claim: *with_claim, state });
                }
                Ok(())
            }
            VerdictAction::Withdraw { gap, .. } => {
                expect_kind(*gap, RecordKind::Gap)?;
                expect_state(
                    *gap,
                    RecordState::Gap(GapState::Registered),
                    self.state_of(*gap).expect("gap exists"),
                )
            }
            VerdictAction::Abandon { hypothesis, .. } => {
                expect_kind(*hypothesis, RecordKind::Hypothesis)?;
                expect_state(
                    *hypothesis,
                    RecordState::Hypothesis(HypothesisState::Registered),
                    self.state_of(*hypothesis).expect("hypothesis exists"),
                )
            }
            VerdictAction::Score { hypothesis, .. } => {
                expect_kind(*hypothesis, RecordKind::Hypothesis)?;
                expect_state(
                    *hypothesis,
                    RecordState::Hypothesis(HypothesisState::Registered),
                    self.state_of(*hypothesis).expect("hypothesis exists"),
                )
            }
        }
    }

    // ── Reads ───────────────────────────────────────────────────────────────

    pub fn record(&self, id: RecordId) -> Option<&Record> {
        self.records.get(&id)
    }

    /// The append log, in order. This *is* the definition of record order.
    pub fn log(&self) -> &[RecordId] {
        &self.log
    }

    /// Where a record sits in the log, in constant-ish time.
    pub fn log_position(&self, id: RecordId) -> Option<usize> {
        self.order.get(&id).copied()
    }

    pub fn records(&self) -> impl Iterator<Item = &Record> {
        self.log.iter().filter_map(|id| self.records.get(id))
    }

    /// Verdicts that touched this record, in log order.
    pub fn history(&self, id: RecordId) -> Vec<&Record> {
        self.by_target
            .get(&id)
            .into_iter()
            .flatten()
            .filter_map(|vid| self.records.get(vid))
            .collect()
    }

    pub fn state_of(&self, id: RecordId) -> Option<RecordState> {
        self.state_folded(id, None)
    }

    /// State as of a record-time: what the ledger said at `at`. Sound because
    /// `append_at` enforces that the log is a prefix of time.
    pub fn state_of_at(&self, id: RecordId, at: Timestamp) -> Option<RecordState> {
        let record = self.records.get(&id)?;
        if record.envelope().recorded_at() > at {
            return None;
        }
        self.state_folded(id, Some(at))
    }

    fn state_folded(&self, id: RecordId, at: Option<Timestamp>) -> Option<RecordState> {
        let record = self.records.get(&id)?;
        let mut state = match record.kind() {
            RecordKind::Claim => RecordState::Claim(ClaimState::Proposed),
            RecordKind::Gap => RecordState::Gap(GapState::Registered),
            RecordKind::Hypothesis => RecordState::Hypothesis(HypothesisState::Registered),
            RecordKind::Verdict => return Some(RecordState::Verdict),
        };
        for verdict in self.history(id) {
            if let Some(at) = at
                && verdict.envelope().recorded_at() > at
            {
                continue;
            }
            let Content::Verdict(v) = verdict.content() else { continue };
            for (target, new_state) in v.action.effects() {
                if target == id {
                    state = new_state;
                }
            }
        }
        Some(state)
    }

    pub fn promoted_claims(&self) -> impl Iterator<Item = &Record> {
        self.records().filter(|r| {
            r.kind() == RecordKind::Claim
                && self.state_of(r.id()) == Some(RecordState::Claim(ClaimState::Promoted))
        })
    }

    /// The records that supersede this one, in log order.
    ///
    /// Usually empty or one. More than one is a fork — two authors each
    /// declaring they replaced the same record — which is legal, surfaced
    /// rather than prevented, and the same posture invariant 7 takes toward
    /// contradictions.
    pub fn replaced_by(&self, id: RecordId) -> &[RecordId] {
        self.replacements.get(&id).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Proposed claims awaiting a verdict — the keeper's inbox, split into
    /// what to read and what a later draft already replaced.
    ///
    /// Only claims need this. A superseded gap is withdrawn and a superseded
    /// hypothesis abandoned (D-0023), so both leave the live set through a
    /// verdict of their own. A claim cannot: the verdict that retires a
    /// predecessor is `Promote { retiring }`, and it can only retire a claim
    /// that reached promoted. One replaced before anyone ruled on it never
    /// did, so nothing closes it — and nothing should, because no one decided
    /// anything about it (U-30).
    pub fn pending_proposals(&self) -> Pending<'_> {
        let mut queued = Vec::new();
        let mut superseded = Vec::new();
        for record in self.records() {
            if record.kind() != RecordKind::Claim
                || self.state_of(record.id()) != Some(RecordState::Claim(ClaimState::Proposed))
            {
                continue;
            }
            // Whatever became of the successor: a rejected replacement does not
            // revive the draft it replaced. The author moved on, and only the
            // author can move back — by proposing again.
            if self.replaced_by(record.id()).is_empty() {
                queued.push(record);
            } else {
                superseded.push(record);
            }
        }
        Pending { queued, superseded }
    }

    /// Registered gaps — the honest boundary of the record.
    pub fn registered_gaps(&self) -> Vec<&Record> {
        self.records()
            .filter(|r| {
                r.kind() == RecordKind::Gap
                    && self.state_of(r.id()) == Some(RecordState::Gap(GapState::Registered))
            })
            .collect()
    }

    pub fn review_queue(&self, now: Timestamp) -> ReviewQueue<'_> {
        let mut due = Vec::new();
        let mut missing_trigger = Vec::new();
        for record in self.promoted_claims() {
            match record.envelope().review_trigger() {
                None => missing_trigger.push(record),
                Some(trigger) => {
                    if let Some(due_at) = trigger.due_at
                        && due_at <= now
                    {
                        due.push(record);
                    }
                }
            }
        }
        ReviewQueue { due, missing_trigger }
    }

    /// Exact-scope contradictions over promoted attribute claims. Relation
    /// cardinality ("can this predicate hold twice?") is semantics, not
    /// grammar — that boundary is U-15.
    pub fn contradictions(&self) -> Vec<Contradiction<'_>> {
        let mut groups: BTreeMap<(EntityId, &str), Vec<&Record>> = BTreeMap::new();
        for record in self.promoted_claims() {
            if let Content::Claim(ClaimContent::Attribute { subject, name, .. }) = record.content()
            {
                groups.entry((*subject, name.as_str())).or_default().push(record);
            }
        }
        let mut found = Vec::new();
        for ((subject, attribute), records) in groups {
            for (i, a) in records.iter().enumerate() {
                for b in &records[i + 1..] {
                    if a.envelope().validity().overlaps(&b.envelope().validity()) {
                        found.push(Contradiction { subject, attribute, a, b });
                    }
                }
            }
        }
        found
    }

    // ── The instrument panel ────────────────────────────────────────────────

    /// Record a machine-owned signal. Takes an explicit clock so that one
    /// logical operation observes one "now" and property tests stay
    /// deterministic.
    pub fn record_measurement(
        &mut self,
        target: MeasurementTarget,
        name: impl Into<String>,
        value: f64,
        updated_by: Author,
        at: Timestamp,
    ) -> Result<(), Error> {
        let name = name.into();
        if self.journal.is_some() {
            // Validate before the write so a rejected measurement never
            // reaches the log.
            self.check_measurement_target(target)?;
            let event = Event::Measurement {
                target,
                name: name.clone(),
                value,
                at,
                by: updated_by.clone(),
            };
            if let Some(journal) = &mut self.journal {
                journal.append(&event)?;
            }
        }
        self.apply_measurement(target, name, value, updated_by, at)
    }

    fn check_measurement_target(&self, target: MeasurementTarget) -> Result<(), Error> {
        match target {
            MeasurementTarget::Entity(e) => {
                if !self.entities.contains_key(&e) {
                    return Err(Error::UnknownEntity(e));
                }
            }
            MeasurementTarget::Relation(r) => {
                let record = self.records.get(&r).ok_or(Error::UnknownRecord(r))?;
                if !matches!(record.content(), Content::Claim(ClaimContent::Relation { .. })) {
                    return Err(Error::MeasurementTargetNotRelation(r));
                }
            }
        }
        Ok(())
    }

    fn apply_measurement(
        &mut self,
        target: MeasurementTarget,
        name: impl Into<String>,
        value: f64,
        updated_by: Author,
        at: Timestamp,
    ) -> Result<(), Error> {
        self.check_measurement_target(target)?;
        let name = name.into();
        self.panel.insert(
            (target, name.clone()),
            Measurement { target, name, value, updated_at: at, updated_by },
        );
        Ok(())
    }

    pub fn measurement(&self, target: MeasurementTarget, name: &str) -> Option<&Measurement> {
        self.panel.get(&(target, name.to_string()))
    }

    pub fn measurements_for(&self, target: MeasurementTarget) -> Vec<&Measurement> {
        self.panel.iter().filter(|((t, _), _)| *t == target).map(|(_, m)| m).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{
        GapContent, HypothesisContent, RetireReason, ScoreOutcome, VerdictContent,
    };
    use crate::envelope::{Evidence, ReviewTrigger, SourceRef};
    use crate::value::Value;

    pub(crate) fn ts(offset: i64) -> Timestamp {
        Timestamp::from_second(1_756_000_000 + offset).unwrap()
    }

    fn setup() -> (Ledger, EntityId, EntityId) {
        let mut ledger = Ledger::new();
        let source = ledger.add_source("founding interview").unwrap();
        let subject = ledger.add_entity("process", "torque check").unwrap();
        (ledger, source, subject)
    }

    fn attribute_claim(subject: EntityId, author: Author) -> Draft {
        Draft::new(
            author,
            SourceRef::channel("interview"),
            Content::Claim(ClaimContent::Attribute {
                subject,
                name: "spec_nm".into(),
                value: Value::Number(24.0),
            }),
        )
    }

    fn verdict(action: VerdictAction, author: Author) -> Draft {
        Draft::new(
            author,
            SourceRef::channel("huddle"),
            Content::Verdict(VerdictContent { action, rationale: None }),
        )
    }

    fn promote(target: RecordId) -> Draft {
        verdict(VerdictAction::Promote { target, retiring: None }, Author::human("Greg"))
    }

    #[test]
    fn claims_start_proposed_and_envelopes_are_sealed() {
        let (mut ledger, _, subject) = setup();
        let id = ledger
            .append_at(attribute_claim(subject, Author::agent("miner")), ts(0))
            .unwrap();
        assert_eq!(ledger.state_of(id), Some(RecordState::Claim(ClaimState::Proposed)));
        let record = ledger.record(id).unwrap();
        assert_eq!(record.envelope().recorded_at(), ts(0));
        assert_eq!(record.envelope().valid_from(), ts(0));
        assert_eq!(record.envelope().version(), crate::envelope::ENVELOPE_VERSION);
    }

    #[test]
    fn human_verdict_promotes_then_retires() {
        let (mut ledger, _, subject) = setup();
        let claim = ledger.append(attribute_claim(subject, Author::agent("miner"))).unwrap();
        ledger.append(promote(claim)).unwrap();
        assert_eq!(ledger.state_of(claim), Some(RecordState::Claim(ClaimState::Promoted)));
        ledger
            .append(verdict(
                VerdictAction::Retire { target: claim, reason: RetireReason::NoLongerTrue },
                Author::human("Greg"),
            ))
            .unwrap();
        assert_eq!(ledger.state_of(claim), Some(RecordState::Claim(ClaimState::Retired)));
        assert_eq!(ledger.history(claim).len(), 2);
    }

    #[test]
    fn agents_cannot_author_verdicts() {
        let (mut ledger, _, subject) = setup();
        let claim = ledger.append(attribute_claim(subject, Author::agent("miner"))).unwrap();
        let err = ledger
            .append(verdict(
                VerdictAction::Promote { target: claim, retiring: None },
                Author::agent("miner"),
            ))
            .unwrap_err();
        assert!(matches!(err, Error::VerdictRequiresHumanAuthor));
        assert_eq!(ledger.state_of(claim), Some(RecordState::Claim(ClaimState::Proposed)));
    }

    #[test]
    fn transitions_are_grammar_checked() {
        let (mut ledger, _, subject) = setup();
        let claim = ledger.append(attribute_claim(subject, Author::human("Greg"))).unwrap();
        ledger.append(promote(claim)).unwrap();
        assert!(matches!(
            ledger.append(promote(claim)).unwrap_err(),
            Error::IllegalTransition { .. }
        ));
        assert!(matches!(
            ledger
                .append(verdict(VerdictAction::Reject { target: claim }, Author::human("Greg")))
                .unwrap_err(),
            Error::IllegalTransition { .. }
        ));
    }

    #[test]
    fn one_verdict_promotes_and_retires_superseded() {
        let (mut ledger, _, subject) = setup();
        let old = ledger.append(attribute_claim(subject, Author::human("Greg"))).unwrap();
        ledger.append(promote(old)).unwrap();
        let mut replacement = attribute_claim(subject, Author::human("Greg"));
        replacement.supersedes = Some(old);
        let new = ledger.append(replacement).unwrap();
        ledger
            .append(verdict(
                VerdictAction::Promote { target: new, retiring: Some(old) },
                Author::human("Greg"),
            ))
            .unwrap();
        assert_eq!(ledger.state_of(new), Some(RecordState::Claim(ClaimState::Promoted)));
        assert_eq!(ledger.state_of(old), Some(RecordState::Claim(ClaimState::Retired)));
        assert_eq!(ledger.record(new).unwrap().envelope().supersedes(), Some(old));
    }

    /// The verdict that promotes one record and retires another writes two
    /// effects; the fold must apply each to its own target and no other.
    #[test]
    fn combined_verdict_effects_do_not_leak_across_targets() {
        let (mut ledger, _, subject) = setup();
        let old = ledger.append(attribute_claim(subject, Author::human("Greg"))).unwrap();
        ledger.append(promote(old)).unwrap();
        let new = ledger.append(attribute_claim(subject, Author::human("Greg"))).unwrap();
        let bystander = ledger.append(attribute_claim(subject, Author::human("Greg"))).unwrap();
        ledger
            .append(verdict(
                VerdictAction::Promote { target: new, retiring: Some(old) },
                Author::human("Greg"),
            ))
            .unwrap();
        assert_eq!(ledger.state_of(new), Some(RecordState::Claim(ClaimState::Promoted)));
        assert_eq!(ledger.state_of(old), Some(RecordState::Claim(ClaimState::Retired)));
        assert_eq!(ledger.state_of(bystander), Some(RecordState::Claim(ClaimState::Proposed)));
    }

    #[test]
    fn promotion_cannot_retire_its_own_target() {
        let (mut ledger, _, subject) = setup();
        let claim = ledger.append(attribute_claim(subject, Author::human("Greg"))).unwrap();
        let err = ledger
            .append(verdict(
                VerdictAction::Promote { target: claim, retiring: Some(claim) },
                Author::human("Greg"),
            ))
            .unwrap_err();
        assert!(matches!(err, Error::PromoteRetireSameRecord));
    }

    #[test]
    fn gaps_answer_only_with_promoted_claims() {
        let (mut ledger, _, subject) = setup();
        let gap = ledger
            .append(Draft::new(
                Author::agent("assistant"),
                SourceRef::channel("chat"),
                Content::Gap(GapContent {
                    question: "what torque for the rail fastener?".into(),
                    territory: vec![subject],
                }),
            ))
            .unwrap();
        let claim = ledger.append(attribute_claim(subject, Author::human("Maria"))).unwrap();

        let err = ledger
            .append(verdict(
                VerdictAction::Answer { gap, with_claim: claim },
                Author::human("Greg"),
            ))
            .unwrap_err();
        assert!(matches!(err, Error::AnswerRequiresPromotedClaim { .. }));

        ledger.append(promote(claim)).unwrap();
        ledger
            .append(verdict(
                VerdictAction::Answer { gap, with_claim: claim },
                Author::human("Greg"),
            ))
            .unwrap();
        assert_eq!(ledger.state_of(gap), Some(RecordState::Gap(GapState::Answered)));
        assert!(ledger.registered_gaps().is_empty());
    }

    #[test]
    fn hypotheses_are_scored() {
        let (mut ledger, _, _) = setup();
        let hyp = ledger
            .append(Draft::new(
                Author::human("Greg"),
                SourceRef::channel("planning"),
                Content::Hypothesis(HypothesisContent {
                    statement: "self-hosting within six months".into(),
                    falsifier: Some("provenance queries still need app-layer workarounds".into()),
                    score_by: ts(1000),
                }),
            ))
            .unwrap();
        ledger
            .append(verdict(
                VerdictAction::Score { hypothesis: hyp, outcome: ScoreOutcome::Met },
                Author::human("Greg"),
            ))
            .unwrap();
        assert_eq!(
            ledger.state_of(hyp),
            Some(RecordState::Hypothesis(HypothesisState::Scored(ScoreOutcome::Met)))
        );
    }

    #[test]
    fn evidence_must_point_at_source_entities() {
        let (mut ledger, source, subject) = setup();
        let mut ok = attribute_claim(subject, Author::human("Greg"));
        ok.evidence.push(Evidence { source, span: Some("p. 3".into()) });
        ledger.append(ok).unwrap();

        let mut bad = attribute_claim(subject, Author::human("Greg"));
        bad.evidence.push(Evidence { source: subject, span: None });
        assert!(matches!(ledger.append(bad).unwrap_err(), Error::EvidenceNotSource(..)));
    }

    #[test]
    fn claims_about_unknown_entities_are_rejected() {
        let (mut ledger, _, subject) = setup();
        let mut other = Ledger::new();
        let foreign = other.add_entity("process", "elsewhere").unwrap();
        let draft = Draft::new(
            Author::human("Greg"),
            SourceRef::channel("interview"),
            Content::Claim(ClaimContent::Relation {
                subject,
                predicate: "feeds".into(),
                object: foreign,
                properties: BTreeMap::new(),
            }),
        );
        assert!(matches!(ledger.append(draft).unwrap_err(), Error::UnknownEntity(_)));
    }

    /// `about` is validated exactly like `GapContent::territory`.
    #[test]
    fn prose_claims_validate_their_entity_refs() {
        let (mut ledger, _, subject) = setup();
        let mut other = Ledger::new();
        let foreign = other.add_entity("decision", "elsewhere").unwrap();

        let good = Draft::new(
            Author::human("Greg"),
            SourceRef::channel("interview"),
            Content::Claim(ClaimContent::Text {
                body: "a claim about the process".into(),
                about: vec![subject],
            }),
        );
        ledger.append(good).unwrap();

        let bad = Draft::new(
            Author::human("Greg"),
            SourceRef::channel("interview"),
            Content::Claim(ClaimContent::Pattern {
                context: "c".into(),
                forces: vec!["f".into()],
                solution: "s".into(),
                about: vec![foreign],
            }),
        );
        assert!(matches!(ledger.append(bad).unwrap_err(), Error::UnknownEntity(_)));
    }

    /// Regression: a repeated ref used to multiply the record across every
    /// entity-scoped read.
    #[test]
    fn repeated_entity_refs_are_rejected() {
        let (mut ledger, _, subject) = setup();
        let prose = Draft::new(
            Author::human("Greg"),
            SourceRef::channel("interview"),
            Content::Claim(ClaimContent::Text {
                body: "a note".into(),
                about: vec![subject, subject],
            }),
        );
        assert!(matches!(ledger.append(prose).unwrap_err(), Error::DuplicateEntityRef(_)));

        let gap = Draft::new(
            Author::agent("assistant"),
            SourceRef::channel("chat"),
            Content::Gap(GapContent { question: "?".into(), territory: vec![subject, subject] }),
        );
        assert!(matches!(ledger.append(gap).unwrap_err(), Error::DuplicateEntityRef(_)));

        // A self-relation is not a duplicate ref: subject and object are
        // distinct roles that happen to name the same entity.
        let loopback = Draft::new(
            Author::human("Greg"),
            SourceRef::channel("interview"),
            Content::Claim(ClaimContent::Relation {
                subject,
                predicate: "depends_on".into(),
                object: subject,
                properties: BTreeMap::new(),
            }),
        );
        ledger.append(loopback).unwrap();
    }

    #[test]
    fn verdict_targets_are_kind_checked() {
        let (mut ledger, _, subject) = setup();
        let gap = ledger
            .append(Draft::new(
                Author::human("Greg"),
                SourceRef::channel("chat"),
                Content::Gap(GapContent { question: "?".into(), territory: vec![subject] }),
            ))
            .unwrap();
        let err = ledger.append(promote(gap)).unwrap_err();
        assert!(matches!(err, Error::WrongTargetKind { .. }));
    }

    #[test]
    fn record_time_travel_reads_past_states() {
        let (mut ledger, _, subject) = setup();
        let claim = ledger
            .append_at(attribute_claim(subject, Author::agent("miner")), ts(10))
            .unwrap();
        ledger.append_at(promote(claim), ts(30)).unwrap();
        assert_eq!(ledger.state_of_at(claim, ts(5)), None);
        assert_eq!(
            ledger.state_of_at(claim, ts(20)),
            Some(RecordState::Claim(ClaimState::Proposed))
        );
        assert_eq!(
            ledger.state_of_at(claim, ts(40)),
            Some(RecordState::Claim(ClaimState::Promoted))
        );
        assert_eq!(ledger.state_of(claim), Some(RecordState::Claim(ClaimState::Promoted)));
    }

    /// `state_of_at` filters by record-time while walking the log in order, so
    /// the log must be a prefix of time or it can report a state that never
    /// existed.
    #[test]
    fn record_time_cannot_move_backwards() {
        let (mut ledger, _, subject) = setup();
        ledger
            .append_at(attribute_claim(subject, Author::human("Greg")), ts(100))
            .unwrap();
        let err = ledger
            .append_at(attribute_claim(subject, Author::human("Greg")), ts(50))
            .unwrap_err();
        assert!(matches!(err, Error::NonMonotonicRecordTime { .. }));
        // Equal timestamps are fine: monotone, not strictly increasing.
        ledger
            .append_at(attribute_claim(subject, Author::human("Greg")), ts(100))
            .unwrap();
    }

    #[test]
    fn record_time_cannot_be_in_the_future() {
        let (mut ledger, _, subject) = setup();
        let future = Timestamp::now() + jiff::SignedDuration::from_hours(24);
        let err = ledger
            .append_at(attribute_claim(subject, Author::human("Greg")), future)
            .unwrap_err();
        assert!(matches!(err, Error::FutureRecordTime { .. }));
    }

    // ── U-28: withdrawing a question, and saying which kind ─────────────────

    fn gap_draft(question: &str) -> Draft {
        Draft::new(
            Author::human("Greg"),
            SourceRef::channel("register"),
            Content::Gap(GapContent { question: question.into(), territory: vec![] }),
        )
    }

    fn hypothesis_draft(statement: &str) -> Draft {
        Draft::new(
            Author::human("Greg"),
            SourceRef::channel("interview"),
            Content::Hypothesis(HypothesisContent {
                statement: statement.into(),
                falsifier: None,
                score_by: ts(10_000),
            }),
        )
    }

    #[test]
    fn a_reworded_question_and_an_abandoned_one_are_different_records() {
        let (mut ledger, _, _) = setup();
        let old = ledger.append(gap_draft("whether a query language is needed")).unwrap();

        let mut replacement = gap_draft("whether a query language is needed, and in what shape");
        replacement.supersedes = Some(old);
        let new = ledger.append(replacement).unwrap();
        ledger
            .append(verdict(
                VerdictAction::Withdraw { gap: old, reason: WithdrawReason::Superseded },
                Author::human("Greg"),
            ))
            .unwrap();

        assert_eq!(ledger.state_of(old), Some(RecordState::Gap(GapState::Withdrawn)));
        assert_eq!(ledger.state_of(new), Some(RecordState::Gap(GapState::Registered)));
        assert_eq!(ledger.record(new).unwrap().envelope().supersedes(), Some(old));
        // One live question, and the successor findable from the record that
        // replaced it rather than from the verdict that closed it.
        let live: Vec<_> = ledger.registered_gaps().iter().map(|r| r.id()).collect();
        assert_eq!(live, vec![new]);
    }

    #[test]
    fn abandoning_a_hypothesis_is_not_scoring_it() {
        let (mut ledger, _, _) = setup();
        let h = ledger.append(hypothesis_draft("within six months it self-hosts")).unwrap();
        ledger
            .append(verdict(
                VerdictAction::Abandon { hypothesis: h, reason: WithdrawReason::NoLongerRelevant },
                Author::human("Greg"),
            ))
            .unwrap();

        // Stopping a prediction and finding it false are different events, and
        // a reader counting falsified hypotheses must not see this one.
        assert_eq!(ledger.state_of(h), Some(RecordState::Hypothesis(HypothesisState::Abandoned)));
        assert_ne!(
            ledger.state_of(h),
            Some(RecordState::Hypothesis(HypothesisState::Scored(ScoreOutcome::Falsified)))
        );
        // And it is spent: a hypothesis leaves the registered set once.
        let err = ledger
            .append(verdict(
                VerdictAction::Score { hypothesis: h, outcome: ScoreOutcome::Met },
                Author::human("Greg"),
            ))
            .unwrap_err();
        assert!(matches!(err, Error::IllegalTransition { .. }), "got {err}");
    }

    #[test]
    fn a_withdrawal_must_say_which_kind_it_is() {
        let (mut ledger, _, _) = setup();
        let gap = ledger.append(gap_draft("whether to shard")).unwrap();
        let err = ledger
            .append(verdict(
                VerdictAction::Withdraw { gap, reason: WithdrawReason::Unstated },
                Author::human("Greg"),
            ))
            .unwrap_err();
        assert!(matches!(err, Error::UnstatedWithdrawReason), "got {err}");
        assert_eq!(ledger.state_of(gap), Some(RecordState::Gap(GapState::Registered)));
    }

    #[test]
    fn supersession_only_means_something_between_records_of_one_kind() {
        let (mut ledger, _, subject) = setup();
        let claim = ledger.append(attribute_claim(subject, Author::human("Greg"))).unwrap();

        let mut gap = gap_draft("whether the torque spec is right");
        gap.supersedes = Some(claim);
        let err = ledger.append(gap).unwrap_err();
        assert!(matches!(err, Error::SupersedesDifferentKind { .. }), "got {err}");

        // The link is where supersession lives, so it has to be trustworthy:
        // a verdict may say "superseded", and this is what makes the successor
        // findable rather than merely asserted.
        let old = ledger.append(gap_draft("whether to shard")).unwrap();
        let mut replacement = gap_draft("whether to shard, and by what key");
        replacement.supersedes = Some(old);
        ledger.append(replacement).expect("a gap may replace a gap");
    }

    // ── U-22: a clock that steps backwards ──────────────────────────────────
    //
    // Both tests start by setting `last_recorded_at` ahead of the wall clock
    // directly. That state cannot be produced through `append_at` — which is
    // exactly the point: no record is ever written ahead of the clock, so the
    // only way the log can lead is for the clock to have moved.

    #[test]
    fn a_small_backwards_clock_step_holds_the_line() {
        let (mut ledger, _, subject) = setup();
        let ahead = Timestamp::now() + SignedDuration::from_secs(2);
        ledger.last_recorded_at = Some(ahead);

        let first = ledger.append(attribute_claim(subject, Author::human("Greg"))).unwrap();
        let second = ledger.append(attribute_claim(subject, Author::human("Greg"))).unwrap();

        // Monotone, not strictly increasing: the line holds rather than the
        // append failing, because what `state_of_at` needs is order, not
        // agreement with the wall clock.
        assert_eq!(ledger.record(first).unwrap().envelope().recorded_at(), ahead);
        assert_eq!(ledger.record(second).unwrap().envelope().recorded_at(), ahead);
        assert_eq!(ledger.clock_holds(), 2);
    }

    #[test]
    fn a_large_backwards_clock_step_refuses_and_names_the_way_out() {
        let (mut ledger, _, subject) = setup();
        let ahead = Timestamp::now() + SignedDuration::from_hours(1);
        ledger.last_recorded_at = Some(ahead);

        let err = ledger.append(attribute_claim(subject, Author::human("Greg"))).unwrap_err();
        assert!(matches!(err, Error::ClockWentBackwards { .. }));
        // U-22 was not "it refuses" — it was "it refuses with no way out".
        assert!(err.to_string().contains("appends resume once it passes"));
        assert_eq!(ledger.clock_holds(), 0);
        assert!(ledger.log().is_empty());

        // And the refusal is a pause, not a brick: the same append succeeds
        // once the clock is back in front of the log.
        ledger.last_recorded_at = Some(Timestamp::now() - SignedDuration::from_secs(1));
        ledger.append(attribute_claim(subject, Author::human("Greg"))).unwrap();
    }

    #[test]
    fn overlapping_promoted_attributes_surface_as_contradictions() {
        let (mut ledger, _, subject) = setup();
        let mut a = attribute_claim(subject, Author::human("Greg"));
        a.valid_from = Some(ts(0));
        let a = ledger.append(a).unwrap();
        ledger.append(promote(a)).unwrap();

        let mut b = attribute_claim(subject, Author::human("Maria"));
        b.valid_from = Some(ts(100));
        let b = ledger.append(b).unwrap();
        ledger.append(promote(b)).unwrap();

        assert_eq!(ledger.contradictions().len(), 1);

        ledger
            .append(verdict(
                VerdictAction::Retire { target: a, reason: RetireReason::Superseded },
                Author::human("Greg"),
            ))
            .unwrap();
        assert!(ledger.contradictions().is_empty());

        // Half-open [from, to): closing C at exactly ts(100) does not overlap
        // B, which starts at ts(100).
        let mut c = attribute_claim(subject, Author::human("Greg"));
        c.valid_from = Some(ts(0));
        c.valid_to = Some(ts(100));
        let c = ledger.append(c).unwrap();
        ledger.append(promote(c)).unwrap();
        assert!(ledger.contradictions().is_empty());
    }

    #[test]
    fn review_queue_flags_due_and_missing_triggers() {
        let (mut ledger, _, subject) = setup();
        let mut with_trigger = attribute_claim(subject, Author::human("Greg"));
        with_trigger.review_trigger = Some(ReviewTrigger { due_at: Some(ts(50)), on_event: None });
        let with_trigger = ledger.append(with_trigger).unwrap();
        ledger.append(promote(with_trigger)).unwrap();

        let bare = ledger.append(attribute_claim(subject, Author::human("Greg"))).unwrap();
        ledger.append(promote(bare)).unwrap();

        let queue = ledger.review_queue(ts(60));
        assert_eq!(queue.due.len(), 1);
        assert_eq!(queue.due[0].id(), with_trigger);
        assert_eq!(queue.missing_trigger.len(), 1);
        assert_eq!(queue.missing_trigger[0].id(), bare);

        let earlier = ledger.review_queue(ts(40));
        assert!(earlier.due.is_empty());
    }

    #[test]
    fn measurements_live_on_the_panel_not_the_ledger() {
        let (mut ledger, _, subject) = setup();
        let object = ledger.add_entity("table", "orders").unwrap();
        let relation = ledger
            .append(Draft::new(
                Author::agent("catalog-sync"),
                SourceRef::channel("ingest"),
                Content::Claim(ClaimContent::Relation {
                    subject,
                    predicate: "joins_to".into(),
                    object,
                    properties: BTreeMap::new(),
                }),
            ))
            .unwrap();

        let target = MeasurementTarget::Relation(relation);
        ledger
            .record_measurement(target, "success_rate", 0.92, Author::agent("updater"), ts(0))
            .unwrap();
        ledger
            .record_measurement(target, "success_rate", 0.95, Author::agent("updater"), ts(1))
            .unwrap();
        assert_eq!(ledger.measurement(target, "success_rate").unwrap().value, 0.95);
        assert_eq!(ledger.measurements_for(target).len(), 1);

        let attribute = ledger.append(attribute_claim(subject, Author::human("Greg"))).unwrap();
        let err = ledger
            .record_measurement(
                MeasurementTarget::Relation(attribute),
                "success_rate",
                1.0,
                Author::agent("updater"),
                ts(2),
            )
            .unwrap_err();
        assert!(matches!(err, Error::MeasurementTargetNotRelation(_)));
    }

    #[test]
    fn inverted_validity_is_rejected() {
        let (mut ledger, _, subject) = setup();
        let mut draft = attribute_claim(subject, Author::human("Greg"));
        draft.valid_from = Some(ts(100));
        draft.valid_to = Some(ts(50));
        assert!(matches!(ledger.append(draft).unwrap_err(), Error::InvalidValidity));
    }

    #[test]
    fn pending_proposals_is_the_keeper_inbox() {
        let (mut ledger, _, subject) = setup();
        let a = ledger.append(attribute_claim(subject, Author::agent("miner"))).unwrap();
        let b = ledger.append(attribute_claim(subject, Author::agent("miner"))).unwrap();
        ledger.append(promote(a)).unwrap();
        let pending: Vec<_> =
            ledger.pending_proposals().queued.iter().map(|r| r.id()).collect();
        assert_eq!(pending, vec![b]);
    }

    // ── U-30: a draft its own author replaced ───────────────────────────────

    #[test]
    fn a_draft_its_author_replaced_is_not_queued_twice() {
        let (mut ledger, _, subject) = setup();
        let first = ledger.append(attribute_claim(subject, Author::agent("miner"))).unwrap();
        let mut second = attribute_claim(subject, Author::agent("miner"));
        second.supersedes = Some(first);
        let second = ledger.append(second).unwrap();
        let mut third = attribute_claim(subject, Author::agent("miner"));
        third.supersedes = Some(second);
        let third = ledger.append(third).unwrap();

        let pending = ledger.pending_proposals();
        // One wording to read, not three.
        assert_eq!(pending.queued.iter().map(|r| r.id()).collect::<Vec<_>>(), vec![third]);
        assert_eq!(
            pending.superseded.iter().map(|r| r.id()).collect::<Vec<_>>(),
            vec![first, second]
        );

        // Nothing was hidden and nothing moved: all three are still proposed,
        // because an author editing an unreviewed draft is not a verdict.
        assert_eq!(pending.all().count(), 3);
        for id in [first, second, third] {
            assert_eq!(ledger.state_of(id), Some(RecordState::Claim(ClaimState::Proposed)));
        }
        assert_eq!(ledger.replaced_by(first), &[second]);
        assert_eq!(ledger.replaced_by(third), &[]);
    }

    #[test]
    fn a_rejected_replacement_does_not_revive_the_draft_it_replaced() {
        let (mut ledger, _, subject) = setup();
        let first = ledger.append(attribute_claim(subject, Author::agent("miner"))).unwrap();
        let mut second = attribute_claim(subject, Author::agent("miner"));
        second.supersedes = Some(first);
        let second = ledger.append(second).unwrap();
        ledger
            .append(verdict(VerdictAction::Reject { target: second }, Author::human("Greg")))
            .unwrap();

        // The reviewer said no to the wording the author stands behind. That is
        // not a yes to the one they abandoned — only the author can put that
        // back, by proposing it again.
        let pending = ledger.pending_proposals();
        assert!(pending.queued.is_empty());
        assert_eq!(pending.superseded.iter().map(|r| r.id()).collect::<Vec<_>>(), vec![first]);
    }

    #[test]
    fn two_drafts_replacing_one_record_are_surfaced_rather_than_prevented() {
        let (mut ledger, _, subject) = setup();
        let original = ledger.append(attribute_claim(subject, Author::agent("miner"))).unwrap();
        let mut a = attribute_claim(subject, Author::agent("miner"));
        a.supersedes = Some(original);
        let a = ledger.append(a).unwrap();
        let mut b = attribute_claim(subject, Author::human("Greg"));
        b.supersedes = Some(original);
        let b = ledger.append(b).unwrap();

        // A fork: two authors each declaring they replaced the same record. Left
        // legal and made visible, the same posture invariant 7 takes toward
        // contradictions — the engine does not get to pick.
        assert_eq!(ledger.replaced_by(original), &[a, b]);
        let pending = ledger.pending_proposals();
        assert_eq!(pending.queued.iter().map(|r| r.id()).collect::<Vec<_>>(), vec![a, b]);
        assert_eq!(pending.superseded.len(), 1);
    }

    #[test]
    fn entity_lookup_deduplicates_by_kind_and_label() {
        let mut ledger = Ledger::new();
        let a = ledger.add_source("docs/REGISTER.md").unwrap();
        let b = ledger.upsert_entity(SOURCE_KIND, "docs/REGISTER.md").unwrap();
        assert_eq!(a, b);
        assert_eq!(ledger.entities().count(), 1);
        assert_eq!(ledger.find_entity("source", "nope"), None);
    }
}

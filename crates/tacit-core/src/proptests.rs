//! Property tests for U-10: the maintained projection index must equal a
//! deterministic rebuild, for every reachable ledger.
//!
//! The op alphabet is abstract — small integer indices, not ids — because
//! `RecordId` and `EntityId` cannot be minted outside the ledger. An
//! interpreter binds each op to real ids against a live ledger, so generated
//! scripts only ever exercise states the engine can actually reach.

use crate::content::{
    ClaimContent, Content, GapContent, RetireReason, VerdictAction, VerdictContent,
};
use crate::envelope::{Author, SourceRef};
use crate::id::{EntityId, RecordId};
use crate::ledger::Ledger;
use crate::projection::{Projection, StateFilter, ViewSpec};
use crate::retrieval::{Outcome, Query, TextIndex};
use crate::record::Draft;
use crate::state::RecordState;
use crate::value::Value;
use jiff::Timestamp;
use proptest::prelude::*;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
enum Op {
    AddEntity,
    Attribute { subject: u8, name: u8, value: i8, from: i16, span: Option<u16> },
    Relation { subject: u8, object: u8, predicate: u8 },
    Prose { about: Option<u8>, pattern: bool },
    Gap { territory: Option<u8> },
    Promote { claim: u8, retiring: Option<u8> },
    Reject { claim: u8 },
    Retire { claim: u8 },
    PromoteSet { first: u8, count: u8 },
    Withdraw { gap: u8 },
}

fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![
        2 => Just(Op::AddEntity),
        6 => (any::<u8>(), 0u8..3, -3i8..3, -50i16..50, proptest::option::of(1u16..80))
            .prop_map(|(subject, name, value, from, span)| Op::Attribute {
                subject, name, value, from, span
            }),
        4 => (any::<u8>(), any::<u8>(), 0u8..3)
            .prop_map(|(subject, object, predicate)| Op::Relation { subject, object, predicate }),
        3 => (proptest::option::of(any::<u8>()), any::<bool>())
            .prop_map(|(about, pattern)| Op::Prose { about, pattern }),
        2 => proptest::option::of(any::<u8>()).prop_map(|territory| Op::Gap { territory }),
        6 => (any::<u8>(), proptest::option::of(any::<u8>()))
            .prop_map(|(claim, retiring)| Op::Promote { claim, retiring }),
        2 => any::<u8>().prop_map(|claim| Op::Reject { claim }),
        3 => any::<u8>().prop_map(|claim| Op::Retire { claim }),
        3 => (any::<u8>(), 1u8..5).prop_map(|(first, count)| Op::PromoteSet { first, count }),
        2 => any::<u8>().prop_map(|gap| Op::Withdraw { gap }),
    ]
}

/// What the temporal reads are *supposed* to say, transcribed from
/// `design/001-data-model.md` §3.1 rather than from the code that answers them.
///
/// The point of writing it out separately is that it is short enough to be
/// obviously right. A record the ledger had not yet recorded has no state; a
/// verdict has none of its own; otherwise start at the kind's opening state and
/// apply, in log order, every verdict recorded by `at`. If the implementation
/// and this disagree, one of them is wrong and the disagreement says where to
/// look — which is what U-14 asked for.
mod reference {
    use super::*;
    use crate::content::SetBasis;
    use crate::state::{ClaimState, GapState, HypothesisState};

    /// Where a verdict puts one record, independent of what state it is in.
    /// The transition table of §3.1, and nothing else.
    fn moves_to(action: &VerdictAction, id: RecordId) -> Option<RecordState> {
        use VerdictAction as A;
        let claim = |s| Some(RecordState::Claim(s));
        match action {
            A::Promote { target, retiring } => {
                if *target == id {
                    claim(ClaimState::Promoted)
                } else if *retiring == Some(id) {
                    claim(ClaimState::Retired)
                } else {
                    None
                }
            }
            A::PromoteSet { targets, retiring, basis } => {
                let _: &SetBasis = basis;
                if targets.contains(&id) {
                    claim(ClaimState::Promoted)
                } else if retiring.contains(&id) {
                    claim(ClaimState::Retired)
                } else {
                    None
                }
            }
            A::Retire { target, .. } if *target == id => claim(ClaimState::Retired),
            A::Reject { target } if *target == id => claim(ClaimState::Rejected),
            A::Answer { gap, .. } if *gap == id => Some(RecordState::Gap(GapState::Answered)),
            A::Withdraw { gap, .. } if *gap == id => {
                Some(RecordState::Gap(GapState::Withdrawn))
            }
            A::Abandon { hypothesis, .. } if *hypothesis == id => {
                Some(RecordState::Hypothesis(HypothesisState::Abandoned))
            }
            A::Score { hypothesis, outcome } if *hypothesis == id => {
                Some(RecordState::Hypothesis(HypothesisState::Scored(*outcome)))
            }
            _ => None,
        }
    }

    fn opening(kind: crate::content::RecordKind) -> RecordState {
        use crate::content::RecordKind as K;
        match kind {
            K::Claim => RecordState::Claim(ClaimState::Proposed),
            K::Gap => RecordState::Gap(GapState::Registered),
            K::Hypothesis => RecordState::Hypothesis(HypothesisState::Registered),
            K::Verdict => RecordState::Verdict,
            K::Redaction => RecordState::Redaction,
        }
    }

    /// The state of `id` as the ledger knew it at `at`.
    pub fn state_at(ledger: &Ledger, id: RecordId, at: Timestamp) -> Option<RecordState> {
        let record = ledger.record(id)?;
        // Not yet learned is not the same as having no state.
        if record.envelope().recorded_at() > at {
            return None;
        }
        if record.kind() == crate::content::RecordKind::Verdict {
            return Some(RecordState::Verdict);
        }
        let mut state = opening(record.kind());
        // Log order is the definition of order, and only what was recorded by
        // `at` may speak.
        for other in ledger.records() {
            if other.envelope().recorded_at() > at {
                continue;
            }
            let Content::Verdict(v) = other.content() else { continue };
            if let Some(next) = moves_to(&v.action, id) {
                state = next;
            }
        }
        Some(state)
    }

    /// Every record-time in the ledger, and the instants either side of each —
    /// where an off-by-one in a boundary lives.
    pub fn interesting_times(ledger: &Ledger) -> Vec<Timestamp> {
        let mut times: Vec<Timestamp> = Vec::new();
        for record in ledger.records() {
            let at = record.envelope().recorded_at();
            times.push(at);
            times.push(at - jiff::SignedDuration::from_nanos(1));
            times.push(at + jiff::SignedDuration::from_nanos(1));
        }
        times.sort_unstable();
        times.dedup();
        times
    }
}

/// Runs a script against a ledger, advancing an incremental projection after
/// every op. Ops that the grammar rejects are simply skipped — an illegal
/// transition is a correct outcome, not a test failure.
struct Interpreter {
    ledger: Ledger,
    incremental: Projection,
    index: TextIndex,
    entities: Vec<EntityId>,
    claims: Vec<RecordId>,
    gaps: Vec<RecordId>,
    clock: i64,
}

impl Interpreter {
    fn new() -> Self {
        Self {
            ledger: Ledger::new(),
            incremental: Projection::empty(),
            index: TextIndex::empty(),
            entities: Vec::new(),
            claims: Vec::new(),
            gaps: Vec::new(),
            clock: 0,
        }
    }

    /// Monotone, never in the future — both `append_at` guards hold.
    fn tick(&mut self) -> Timestamp {
        self.clock += 1;
        Timestamp::from_second(1_700_000_000 + self.clock).unwrap()
    }

    fn pick<T: Copy>(pool: &[T], ix: u8) -> Option<T> {
        if pool.is_empty() { None } else { Some(pool[ix as usize % pool.len()]) }
    }

    fn run(&mut self, ops: &[Op]) {
        for op in ops {
            self.step(op);
            self.incremental.advance(&self.ledger);
            self.index.advance(&self.ledger);
        }
    }

    fn step(&mut self, op: &Op) {
        let at = self.tick();
        match op {
            Op::AddEntity => {
                let n = self.entities.len();
                let id = self.ledger.add_entity("station", format!("E{n}")).unwrap();
                self.entities.push(id);
            }
            Op::Attribute { subject, name, value, from, span } => {
                let Some(subject) = Self::pick(&self.entities, *subject) else { return };
                let valid_from = Timestamp::from_second(1_600_000_000 + i64::from(*from)).unwrap();
                let valid_to = span.map(|s| {
                    Timestamp::from_second(1_600_000_000 + i64::from(*from) + i64::from(s)).unwrap()
                });
                let mut draft = Draft::new(
                    Author::agent("gen"),
                    SourceRef::channel("proptest"),
                    Content::Claim(ClaimContent::Attribute {
                        subject,
                        name: format!("attr{name}"),
                        value: Value::Integer(i64::from(*value)),
                    }),
                );
                draft.valid_from = Some(valid_from);
                draft.valid_to = valid_to;
                if let Ok(id) = self.ledger.append_at(draft, at) {
                    self.claims.push(id);
                }
            }
            Op::Relation { subject, object, predicate } => {
                let (Some(subject), Some(object)) =
                    (Self::pick(&self.entities, *subject), Self::pick(&self.entities, *object))
                else {
                    return;
                };
                let draft = Draft::new(
                    Author::agent("gen"),
                    SourceRef::channel("proptest"),
                    Content::Claim(ClaimContent::Relation {
                        subject,
                        predicate: format!("p{predicate}"),
                        object,
                        properties: BTreeMap::new(),
                    }),
                );
                if let Ok(id) = self.ledger.append_at(draft, at) {
                    self.claims.push(id);
                }
            }
            Op::Prose { about, pattern } => {
                let about: Vec<EntityId> =
                    about.and_then(|ix| Self::pick(&self.entities, ix)).into_iter().collect();
                let content = if *pattern {
                    ClaimContent::Pattern {
                        context: "ctx".into(),
                        forces: vec!["f".into()],
                        solution: "s".into(),
                        about,
                    }
                } else {
                    ClaimContent::Text { body: "body".into(), about }
                };
                let draft = Draft::new(
                    Author::agent("gen"),
                    SourceRef::channel("proptest"),
                    Content::Claim(content),
                );
                if let Ok(id) = self.ledger.append_at(draft, at) {
                    self.claims.push(id);
                }
            }
            Op::Gap { territory } => {
                let territory: Vec<EntityId> =
                    territory.and_then(|ix| Self::pick(&self.entities, ix)).into_iter().collect();
                let draft = Draft::new(
                    Author::agent("gen"),
                    SourceRef::channel("proptest"),
                    Content::Gap(GapContent { question: "q".into(), territory }),
                );
                if let Ok(id) = self.ledger.append_at(draft, at) {
                    self.gaps.push(id);
                }
            }
            Op::Promote { claim, retiring } => {
                let Some(target) = Self::pick(&self.claims, *claim) else { return };
                let retiring = retiring
                    .and_then(|ix| Self::pick(&self.claims, ix))
                    .filter(|r| *r != target);
                self.verdict(VerdictAction::Promote { target, retiring }, at);
            }
            Op::Reject { claim } => {
                let Some(target) = Self::pick(&self.claims, *claim) else { return };
                self.verdict(VerdictAction::Reject { target }, at);
            }
            Op::PromoteSet { first, count } => {
                if self.claims.is_empty() {
                    return;
                }
                let start = *first as usize % self.claims.len();
                let mut targets: Vec<RecordId> = Vec::new();
                for i in 0..*count as usize {
                    let id = self.claims[(start + i) % self.claims.len()];
                    if !targets.contains(&id) {
                        targets.push(id);
                    }
                }
                self.verdict(
                    VerdictAction::PromoteSet {
                        targets,
                        retiring: Vec::new(),
                        basis: crate::content::SetBasis::Ingestion,
                    },
                    at,
                );
            }
            Op::Withdraw { gap } => {
                let Some(target) = Self::pick(&self.gaps, *gap) else { return };
                self.verdict(
                    VerdictAction::Withdraw {
                        gap: target,
                        reason: crate::content::WithdrawReason::NoLongerRelevant,
                    },
                    at,
                );
            }
            Op::Retire { claim } => {
                let Some(target) = Self::pick(&self.claims, *claim) else { return };
                self.verdict(
                    VerdictAction::Retire { target, reason: RetireReason::NoLongerTrue },
                    at,
                );
            }
        }
    }

    fn verdict(&mut self, action: VerdictAction, at: Timestamp) {
        let draft = Draft::new(
            Author::human("Greg"),
            SourceRef::channel("huddle"),
            Content::Verdict(VerdictContent { action, rationale: None }),
        );
        let _ = self.ledger.append_at(draft, at);
    }
}

proptest! {
    /// U-10's core claim: interleaving `advance` with `append` yields exactly
    /// the index a single end-to-end fold would produce.
    #[test]
    fn incremental_equals_rebuild(ops in prop::collection::vec(op_strategy(), 0..60)) {
        let mut interp = Interpreter::new();
        interp.run(&ops);
        prop_assert_eq!(&interp.incremental, &Projection::rebuild(&interp.ledger));
    }

    /// A redundant advance is a no-op, so callers cannot corrupt the index by
    /// advancing more often than they appended.
    #[test]
    fn advance_is_idempotent(ops in prop::collection::vec(op_strategy(), 0..40)) {
        let mut interp = Interpreter::new();
        interp.run(&ops);
        let before = interp.incremental.clone();
        prop_assert_eq!(interp.incremental.advance(&interp.ledger), 0);
        prop_assert_eq!(&before, &interp.incremental);
    }

    /// The index's state map must agree with the ledger's verdict fold for
    /// every claim, whatever its content shape.
    #[test]
    fn index_state_agrees_with_ledger(ops in prop::collection::vec(op_strategy(), 0..60)) {
        let mut interp = Interpreter::new();
        interp.run(&ops);
        for record in interp.ledger.records() {
            if let Some(RecordState::Claim(expected)) = interp.ledger.state_of(record.id()) {
                prop_assert_eq!(interp.incremental.state_of(record.id()), Some(expected));
            }
        }
    }

    /// Views are pure reads: constructing and traversing any view leaves the
    /// index byte-identical.
    #[test]
    fn views_never_mutate_the_index(ops in prop::collection::vec(op_strategy(), 0..40)) {
        let mut interp = Interpreter::new();
        interp.run(&ops);
        let before = interp.incremental.clone();
        for filter in [StateFilter::Promoted, StateFilter::PromotedAndProposed, StateFilter::All] {
            let spec = ViewSpec::now().with_states(filter);
            let view = interp.incremental.view(&interp.ledger, spec);
            for node in view.nodes() {
                let _ = node.properties();
                let _ = node.out_edges();
                let _ = node.in_edges();
                let _ = node.about();
            }
            let _ = view.conflicts();
            let _ = view.edges();
        }
        prop_assert_eq!(&before, &interp.incremental);
    }

    /// Every edge and property a default view admits is promoted, and every
    /// conflict it reports really has two or more admitted claims.
    #[test]
    fn default_view_admits_only_promoted(ops in prop::collection::vec(op_strategy(), 0..60)) {
        let mut interp = Interpreter::new();
        interp.run(&ops);
        let view = interp.incremental.view(&interp.ledger, ViewSpec::now());
        for edge in view.edges() {
            prop_assert_eq!(edge.state(), crate::state::ClaimState::Promoted);
        }
        for node in view.nodes() {
            for (_, property) in node.properties() {
                for claim in property.claims() {
                    prop_assert_eq!(claim.state(), crate::state::ClaimState::Promoted);
                }
            }
        }
        for (_, _, property) in view.conflicts() {
            prop_assert!(property.claims().len() >= 2);
            prop_assert!(property.is_conflicted());
        }
    }
}

proptest! {
    /// The text index is a derived artifact under the same discipline as the
    /// projection: interleaved maintenance equals a single end-to-end fold.
    #[test]
    fn index_incremental_equals_rebuild(ops in prop::collection::vec(op_strategy(), 0..60)) {
        let mut interp = Interpreter::new();
        interp.run(&ops);
        prop_assert_eq!(&interp.index, &TextIndex::rebuild(&interp.ledger));
        prop_assert_eq!(interp.index.advance(&interp.ledger), 0);
    }

    /// The same equivalence for the vector index, which now carries
    /// neighbourhood buckets as well as vectors (D-0032).
    ///
    /// This is the invariant that chose the index: a signature depends on its
    /// own vector and nothing else, so folding a record in later lands it in
    /// exactly the bucket a rebuild would. A navigable graph whose edges depend
    /// on insertion order, or cells whose centroids move as data arrives, could
    /// not have passed this.
    #[test]
    fn vector_index_incremental_equals_rebuild(ops in prop::collection::vec(op_strategy(), 0..40)) {
        let mut interp = Interpreter::new();
        let embedder = crate::embedding::HashingEmbedder::default();
        let model = crate::embedding::Embedder::model_id(&embedder).to_string();
        let mut incremental =
            crate::embedding::VectorIndex::empty(&model).with_neighbourhoods();
        for op in &ops {
            interp.run(std::slice::from_ref(op));
            incremental.advance(&interp.ledger, &embedder);
        }
        prop_assert_eq!(
            &incremental,
            &crate::embedding::VectorIndex::rebuild_searchable(&interp.ledger, &embedder)
        );
        prop_assert_eq!(incremental.advance(&interp.ledger, &embedder), 0);
    }

    // ── U-14: the temporal reads, against a reference semantics ─────────────

    /// The read the whole bitemporal claim rests on, checked against the
    /// definition at every record-time in the ledger and the instants either
    /// side of each — which is where an off-by-one in a boundary lives.
    #[test]
    fn state_of_at_agrees_with_the_reference(ops in prop::collection::vec(op_strategy(), 0..50)) {
        let mut interp = Interpreter::new();
        interp.run(&ops);
        let ids: Vec<RecordId> = interp.ledger.log().to_vec();
        for at in reference::interesting_times(&interp.ledger) {
            for id in &ids {
                prop_assert_eq!(
                    interp.ledger.state_of_at(*id, at),
                    reference::state_at(&interp.ledger, *id, at),
                    "at {} for {}", at, id
                );
            }
        }
    }

    /// `state_of` is not a separate mechanism; it is the temporal read at now.
    /// If these could disagree, every current answer would be a second opinion.
    #[test]
    fn the_current_state_is_the_temporal_read_at_now(ops in prop::collection::vec(op_strategy(), 0..40)) {
        let mut interp = Interpreter::new();
        interp.run(&ops);
        let now = Timestamp::now();
        for id in interp.ledger.log() {
            prop_assert_eq!(
                interp.ledger.state_of(*id),
                interp.ledger.state_of_at(*id, now)
            );
        }
    }

    /// Knowledge only accumulates. A record the ledger knew about at one moment
    /// is known at every later one — history is never rewritten, so nothing can
    /// vanish from it.
    #[test]
    fn knowledge_only_accumulates(ops in prop::collection::vec(op_strategy(), 0..40)) {
        let mut interp = Interpreter::new();
        interp.run(&ops);
        let times = reference::interesting_times(&interp.ledger);
        for id in interp.ledger.log() {
            let mut seen = false;
            for at in &times {
                let known = interp.ledger.state_of_at(*id, *at).is_some();
                if seen {
                    prop_assert!(known, "{} vanished from history at {}", id, at);
                }
                seen |= known;
            }
        }
    }

    /// Recorded-at is the instant a record becomes visible: invisible the
    /// nanosecond before, visible at it. Both boundaries are inclusive of the
    /// same instant, and a mismatch here is what makes a same-tick verdict
    /// flip a test under load.
    #[test]
    fn a_record_appears_exactly_when_it_was_recorded(ops in prop::collection::vec(op_strategy(), 0..30)) {
        let mut interp = Interpreter::new();
        interp.run(&ops);
        for id in interp.ledger.log() {
            let at = interp.ledger.record(*id).unwrap().envelope().recorded_at();
            prop_assert!(interp.ledger.state_of_at(*id, at).is_some(), "{} at its own record-time", id);
            prop_assert!(
                interp.ledger
                    .state_of_at(*id, at - jiff::SignedDuration::from_nanos(1))
                    .is_none(),
                "{} was visible before it was recorded", id
            );
        }
    }

    /// One state, two readers. A view at record-time `t` must admit only what
    /// the ledger says was true at `t` — the projection keeps its own answer
    /// and the two agreeing is not something to assume.
    #[test]
    fn a_view_admits_only_what_the_ledger_says_at_that_record_time(
        ops in prop::collection::vec(op_strategy(), 0..40)
    ) {
        let mut interp = Interpreter::new();
        interp.run(&ops);
        for at in reference::interesting_times(&interp.ledger) {
            // Both axes at the same instant: the ordinary as-of query.
            let spec = ViewSpec::at(at);
            let view = interp.incremental.view(&interp.ledger, spec);
            for id in interp.ledger.log() {
                if !view.admits_record(*id) {
                    continue;
                }
                let state = interp.ledger.state_of_at(*id, at);
                prop_assert!(
                    matches!(
                        state,
                        Some(RecordState::Claim(crate::state::ClaimState::Promoted))
                            | Some(RecordState::Gap(crate::state::GapState::Registered))
                            | Some(RecordState::Hypothesis(
                                crate::state::HypothesisState::Registered
                            ))
                    ),
                    "the default view admitted {} at {} whose state then was {:?}",
                    id, at, state
                );
            }
        }
    }

    /// A view over a projection that has not been advanced must still agree
    /// with the ledger. The index is an optimisation of the fold, and an
    /// optimisation that answers differently is not one.
    #[test]
    fn a_stale_projection_still_agrees_with_the_ledger(
        first in prop::collection::vec(op_strategy(), 0..25),
        then in prop::collection::vec(op_strategy(), 1..25),
    ) {
        let mut interp = Interpreter::new();
        interp.run(&first);
        // Taken here and held while the world moves on.
        let stale = interp.incremental.clone();
        interp.run(&then);

        for at in reference::interesting_times(&interp.ledger) {
            let view = stale.view(&interp.ledger, ViewSpec::at(at));
            for id in interp.ledger.log() {
                if !view.admits_record(*id) {
                    continue;
                }
                prop_assert!(
                    interp.ledger.state_of_at(*id, at).is_some(),
                    "a stale view admitted {} at {}, which the ledger had not recorded", id, at
                );
                if interp.ledger.record(*id).unwrap().kind() == crate::content::RecordKind::Claim {
                    prop_assert_eq!(
                        interp.ledger.state_of_at(*id, at),
                        Some(RecordState::Claim(crate::state::ClaimState::Promoted)),
                        "a stale view admitted {} at {} on a state the ledger disagrees with", id, at
                    );
                }
            }
        }
    }

    /// The headline claim, with the axes pulled apart: "what did the record say
    /// at T1 about what was true at T2". Every property above ties them to one
    /// instant, which exercises the cross-product only by accident.
    #[test]
    fn the_two_axes_hold_independently(
        ops in prop::collection::vec(op_strategy(), 0..40),
        record_ix in 0usize..64,
        valid_ix in 0usize..64,
    ) {
        let mut interp = Interpreter::new();
        interp.run(&ops);
        let times = reference::interesting_times(&interp.ledger);
        if times.is_empty() {
            return Ok(());
        }
        let record_time = times[record_ix % times.len()];
        let valid_at = times[valid_ix % times.len()];

        let view = interp
            .incremental
            .view(&interp.ledger, ViewSpec::bitemporal(record_time, valid_at));
        for id in interp.ledger.log() {
            if !view.admits_record(*id) {
                continue;
            }
            let record = interp.ledger.record(*id).unwrap();
            // Three independent conditions, each on its own axis.
            prop_assert!(
                record.envelope().recorded_at() <= record_time,
                "{} admitted at a record-time before it existed", id
            );
            prop_assert!(
                record.envelope().validity().contains(valid_at),
                "{} admitted at {} which its validity does not contain", id, valid_at
            );
            prop_assert!(
                interp.ledger.state_of_at(*id, record_time).is_some(),
                "{} admitted with no state at {}", id, record_time
            );
        }
    }

    /// Valid-time is one half-open interval `[from, to)` with one definition,
    /// and the two readers of it must agree: two intervals overlap exactly when
    /// some instant falls in both.
    #[test]
    fn overlap_is_exactly_a_shared_instant(
        a in (0i64..40, proptest::option::of(0i64..40)),
        b in (0i64..40, proptest::option::of(0i64..40)),
        probes in prop::collection::vec(0i64..40, 1..12),
    ) {
        let make = |(from, span): (i64, Option<i64>)| {
            let start = Timestamp::from_second(1_700_000_000 + from).unwrap();
            crate::validity::Validity::new(
                start,
                span.map(|s| start + jiff::SignedDuration::from_secs(s)),
            )
        };
        let (Some(a), Some(b)) = (make(a), make(b)) else { return Ok(()) };

        // Symmetric, and every containment is inside the declared bounds.
        prop_assert_eq!(a.overlaps(&b), b.overlaps(&a));
        prop_assert!(a.contains(a.from()));
        if let Some(end) = a.to() {
            prop_assert!(!a.contains(end), "half-open at the far end");
        }

        // A shared instant implies overlap. The converse needs a witness, so it
        // is sampled rather than claimed.
        for p in probes {
            let t = Timestamp::from_second(1_700_000_000 + p).unwrap();
            if a.contains(t) && b.contains(t) {
                prop_assert!(a.overlaps(&b), "{:?} and {:?} share {}", a, b, t);
            }
        }
    }

    /// Retrieval is a pure read and never outruns its budget; and whatever the
    /// default view returns, it returns only records that view admits.
    #[test]
    fn retrieval_respects_budget_and_filter(ops in prop::collection::vec(op_strategy(), 0..50)) {
        let mut interp = Interpreter::new();
        interp.run(&ops);
        let before = interp.index.clone();
        let spec = ViewSpec::now();
        let retriever = interp.index.retriever(&interp.ledger, &interp.incremental, spec);
        let view = retriever.view();

        for text in ["attr0", "ctx body q", "p0 f s"] {
            let found = retriever.retrieve(&Query::text(text));
            prop_assert!(found.items.len() <= 10);
            for item in &found.items {
                prop_assert!(
                    view.admits_record(item.record.id()),
                    "returned a record the view does not admit"
                );
            }
            for gap in &found.gaps {
                prop_assert!(view.admits_record(gap.id()));
            }
            // An empty result set is exactly the None outcome, never a silent
            // claim of confidence.
            prop_assert_eq!(found.items.is_empty(), found.outcome == Outcome::None);
        }
        prop_assert_eq!(&before, &interp.index);
    }
}

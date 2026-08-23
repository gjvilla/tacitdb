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
use crate::ledger::MemoryLedger;
use crate::projection::{Projection, StateFilter, ViewSpec};
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
    ]
}

/// Runs a script against a ledger, advancing an incremental projection after
/// every op. Ops that the grammar rejects are simply skipped — an illegal
/// transition is a correct outcome, not a test failure.
struct Interpreter {
    ledger: MemoryLedger,
    incremental: Projection,
    entities: Vec<EntityId>,
    claims: Vec<RecordId>,
    clock: i64,
}

impl Interpreter {
    fn new() -> Self {
        Self {
            ledger: MemoryLedger::new(),
            incremental: Projection::empty(),
            entities: Vec::new(),
            claims: Vec::new(),
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
        }
    }

    fn step(&mut self, op: &Op) {
        let at = self.tick();
        match op {
            Op::AddEntity => {
                let n = self.entities.len();
                let id = self.ledger.add_entity("station", format!("E{n}"));
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
                let _ = self.ledger.append_at(draft, at);
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

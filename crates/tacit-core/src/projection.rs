//! The traversable current graph (design/001 §5) as a *view*, never a store.
//!
//! Two types carry the design:
//!
//! - [`Projection`] is a **candidate index**: a pure fold over the ledger log
//!   that holds no view parameters at all — no valid-time, no state filter, no
//!   record-time. Nothing is ever removed from it; retirement and expiry are
//!   read-time predicates. That is what makes incremental maintenance monotone
//!   and its equivalence to rebuild definitional rather than hoped for (U-10):
//!   `rebuild` *is* `empty().advance()`, one fold, one cursor.
//! - [`GraphView`] pairs an index, a ledger, and a fully pinned [`ViewSpec`].
//!   Every filter is applied here, so changing valid-time, state filter, or
//!   author filter costs nothing and cannot desynchronize anything.
//!
//! Time is a read parameter and never engine state. Validity expiry — a claim
//! leaving the graph because time passed, with no append — therefore cannot
//! make the index stale; there is nothing time-dependent in it to go stale.
//!
//! The projection is a value the caller holds. The ledger does not own one and
//! the write path holds no reference to one, so no verdict can ever be
//! validated against a stale view.

use crate::content::{ClaimContent, Content};
use crate::entity::Entity;
use crate::envelope::{AuthorKind, Envelope};
use crate::error::Error;
use crate::id::{EntityId, RecordId};
use crate::ledger::MemoryLedger;
use crate::measurement::{Measurement, MeasurementTarget};
use crate::record::Record;
use crate::state::{ClaimState, GapState, RecordState};
use crate::validity::Validity;
use crate::value::Value;
use jiff::Timestamp;
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

// ── View specification ──────────────────────────────────────────────────────

/// Which claim states a view admits. Projected elements always report their
/// own state, so a non-default view labels rather than lies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum StateFilter {
    /// The default knowledge graph.
    Promoted,
    /// The keeper's "what if the queue were promoted" view.
    PromotedAndProposed,
    /// Forensic: every claim ever recorded, retired and rejected included.
    All,
}

impl StateFilter {
    pub fn admits(self, state: ClaimState) -> bool {
        match self {
            StateFilter::Promoted => state == ClaimState::Promoted,
            StateFilter::PromotedAndProposed => {
                matches!(state, ClaimState::Promoted | ClaimState::Proposed)
            }
            StateFilter::All => true,
        }
    }
}

/// A fully pinned view. Both temporal axes are explicit timestamps — there is
/// no `None` meaning "now" — so a view cannot go stale underneath a reader and
/// one traversal cannot straddle two instants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ViewSpec {
    record_time: Timestamp,
    valid_at: Timestamp,
    states: StateFilter,
    author_kind: Option<AuthorKind>,
}

impl ViewSpec {
    /// The default view: record-time = now, valid-time = now, promoted only.
    /// Reads the clock exactly once so both axes agree.
    pub fn now() -> Self {
        Self::at(Timestamp::now())
    }

    /// Both axes at one instant — the usual as-of query.
    pub fn at(instant: Timestamp) -> Self {
        Self {
            record_time: instant,
            valid_at: instant,
            states: StateFilter::Promoted,
            author_kind: None,
        }
    }

    /// The two axes independently: "what did the record say at `record_time`
    /// about what was true at `valid_at`".
    pub fn bitemporal(record_time: Timestamp, valid_at: Timestamp) -> Self {
        Self { record_time, valid_at, states: StateFilter::Promoted, author_kind: None }
    }

    pub fn with_states(mut self, states: StateFilter) -> Self {
        self.states = states;
        self
    }

    pub fn by_author_kind(mut self, kind: AuthorKind) -> Self {
        self.author_kind = Some(kind);
        self
    }

    pub fn record_time(&self) -> Timestamp {
        self.record_time
    }

    pub fn valid_at(&self) -> Timestamp {
        self.valid_at
    }

    pub fn states(&self) -> StateFilter {
        self.states
    }

    pub fn author_kind(&self) -> Option<AuthorKind> {
        self.author_kind
    }

    pub fn is_default(&self) -> bool {
        self.states == StateFilter::Promoted && self.author_kind.is_none()
    }
}

// ── The maintained candidate index ──────────────────────────────────────────

/// Immutable envelope facts denormalized out of a claim record. Safe to copy
/// because envelopes are sealed; the one mutable fact (state) is not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Slot {
    record: RecordId,
    /// Log position — the only stable ordering. `RecordId` is a ULID and is
    /// not monotonic within a millisecond, so it must never stand in for this.
    seq: u64,
    recorded_at: Timestamp,
    validity: Validity,
    author_kind: AuthorKind,
}

impl Slot {
    fn of(record: &Record, seq: u64) -> Self {
        Self {
            record: record.id(),
            seq,
            recorded_at: record.envelope().recorded_at(),
            validity: record.envelope().validity(),
            author_kind: record.envelope().author().kind,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EdgeSlot {
    slot: Slot,
    other: EntityId,
}

/// A fold over the ledger log, carrying no view parameters. Deliberately not
/// `Serialize`: it is a derived artifact, rebuildable from the ledger, and a
/// wire form would invite treating it as authoritative.
#[derive(Debug, Clone, PartialEq)]
pub struct Projection {
    applied: usize,
    seq: u64,
    frontier: Option<Timestamp>,
    states: BTreeMap<RecordId, ClaimState>,
    attributes: BTreeMap<(EntityId, String), Vec<Slot>>,
    out_edges: BTreeMap<EntityId, Vec<EdgeSlot>>,
    in_edges: BTreeMap<EntityId, Vec<EdgeSlot>>,
    /// Prose claims (Pattern/Text) indexed by the entities they are `about`,
    /// and gaps by their territory — the only way entity-scoped reads can
    /// reach non-attribute content.
    about: BTreeMap<EntityId, Vec<Slot>>,
}

impl Default for Projection {
    fn default() -> Self {
        Self::empty()
    }
}

impl Projection {
    pub fn empty() -> Self {
        Self {
            applied: 0,
            seq: 0,
            frontier: None,
            states: BTreeMap::new(),
            attributes: BTreeMap::new(),
            out_edges: BTreeMap::new(),
            in_edges: BTreeMap::new(),
            about: BTreeMap::new(),
        }
    }

    /// The canonical deterministic rebuild — literally `empty().advance()`.
    pub fn rebuild(ledger: &MemoryLedger) -> Self {
        let mut projection = Self::empty();
        projection.advance(ledger);
        projection
    }

    /// Fold the log suffix this index has not seen. A no-op when nothing was
    /// appended. Returns the number of records consumed.
    ///
    /// Owning the cursor is deliberate: an `apply(record)` that trusted the
    /// caller to feed each record exactly once in log order would make
    /// double-apply and skipped-record silent corruption.
    pub fn advance(&mut self, ledger: &MemoryLedger) -> usize {
        let log = ledger.log();
        debug_assert!(
            log.len() >= self.applied,
            "projection advanced against a shorter log — wrong ledger"
        );
        let start = self.applied;
        for id in &log[start..] {
            let record = ledger.record(*id).expect("log entries resolve");
            self.step(record);
        }
        self.applied = log.len();
        self.applied - start
    }

    /// The only mutation path; `rebuild` and `advance` share it. Nothing is
    /// ever removed here — that is the monotonicity U-10 rests on.
    fn step(&mut self, record: &Record) {
        let seq = self.seq;
        self.seq += 1;
        self.frontier = Some(match self.frontier {
            Some(f) if f > record.envelope().recorded_at() => f,
            _ => record.envelope().recorded_at(),
        });

        match record.content() {
            Content::Claim(claim) => {
                let slot = Slot::of(record, seq);
                // Every claim gets a state entry, whatever its shape.
                self.states.insert(record.id(), ClaimState::Proposed);
                match claim {
                    ClaimContent::Attribute { subject, name, .. } => {
                        self.attributes.entry((*subject, name.clone())).or_default().push(slot);
                    }
                    ClaimContent::Relation { subject, object, .. } => {
                        self.out_edges
                            .entry(*subject)
                            .or_default()
                            .push(EdgeSlot { slot, other: *object });
                        self.in_edges
                            .entry(*object)
                            .or_default()
                            .push(EdgeSlot { slot, other: *subject });
                    }
                    ClaimContent::Pattern { about, .. } | ClaimContent::Text { about, .. } => {
                        for entity in about {
                            self.about.entry(*entity).or_default().push(slot);
                        }
                    }
                }
            }
            Content::Gap(gap) => {
                let slot = Slot::of(record, seq);
                for entity in &gap.territory {
                    self.about.entry(*entity).or_default().push(slot);
                }
            }
            Content::Hypothesis(_) => {}
            Content::Verdict(v) => {
                for (target, new_state) in v.action.effects() {
                    if let RecordState::Claim(state) = new_state {
                        self.states.insert(target, state);
                    }
                }
            }
        }
    }

    /// Highest record-time folded so far. A `ViewSpec` at or after this uses
    /// the fast path; before it, state resolves through the ledger.
    pub fn frontier(&self) -> Option<Timestamp> {
        self.frontier
    }

    /// How many log records have been folded.
    pub fn applied(&self) -> usize {
        self.applied
    }

    /// Current state as the fold sees it. Must equal `ledger.state_of(id)`.
    pub fn state_of(&self, id: RecordId) -> Option<ClaimState> {
        self.states.get(&id).copied()
    }

    /// Pair with a ledger and a view spec. Free — no allocation, no rebuild.
    pub fn view<'a>(&'a self, ledger: &'a MemoryLedger, spec: ViewSpec) -> GraphView<'a> {
        GraphView { ledger, projection: self, spec }
    }
}

// ── The read surface ────────────────────────────────────────────────────────

/// A transient pairing of index, ledger, and view spec.
#[derive(Debug, Clone, Copy)]
pub struct GraphView<'a> {
    ledger: &'a MemoryLedger,
    projection: &'a Projection,
    spec: ViewSpec,
}

impl<'a> GraphView<'a> {
    pub fn spec(&self) -> ViewSpec {
        self.spec
    }

    fn state_at(&self, record: RecordId) -> Option<ClaimState> {
        let past = self.projection.frontier.is_some_and(|f| self.spec.record_time < f);
        if past {
            match self.ledger.state_of_at(record, self.spec.record_time) {
                Some(RecordState::Claim(state)) => Some(state),
                _ => None,
            }
        } else {
            self.projection.state_of(record)
        }
    }

    fn admits(&self, slot: &Slot) -> bool {
        if slot.recorded_at > self.spec.record_time {
            return false;
        }
        if let Some(kind) = self.spec.author_kind
            && slot.author_kind != kind
        {
            return false;
        }
        if !slot.validity.contains(self.spec.valid_at) {
            return false;
        }
        self.state_at(slot.record).is_some_and(|s| self.spec.states.admits(s))
    }

    /// Nodes are entities — including dark ones with no admitted claims,
    /// because identity is not claim-derived and hiding it would make the
    /// graph lie about what exists.
    pub fn node(&self, id: EntityId) -> Option<Node<'a>> {
        let entity = self.ledger.entity(id)?;
        Some(Node { view: *self, entity })
    }

    pub fn nodes(&self) -> Vec<Node<'a>> {
        self.ledger.entities().map(|entity| Node { view: *self, entity }).collect()
    }

    pub fn edge(&self, record: RecordId) -> Option<Edge<'a>> {
        let rec = self.ledger.record(record)?;
        let Content::Claim(ClaimContent::Relation { subject, predicate, object, properties }) =
            rec.content()
        else {
            return None;
        };
        let slot = Slot::of(rec, 0);
        if !self.admits(&slot) {
            return None;
        }
        Some(Edge {
            record,
            subject: *subject,
            predicate,
            object: *object,
            properties,
            envelope: rec.envelope(),
            state: self.state_at(record)?,
            view: *self,
        })
    }

    pub fn edges(&self) -> Vec<Edge<'a>> {
        let mut edges: Vec<Edge<'a>> = self
            .projection
            .out_edges
            .values()
            .flatten()
            .filter(|e| self.admits(&e.slot))
            .filter_map(|e| self.edge_from_slot(&e.slot))
            .collect();
        edges.sort_by_key(|e| e.seq());
        edges
    }

    fn edge_from_slot(&self, slot: &Slot) -> Option<Edge<'a>> {
        let rec = self.ledger.record(slot.record)?;
        let Content::Claim(ClaimContent::Relation { subject, predicate, object, properties }) =
            rec.content()
        else {
            return None;
        };
        Some(Edge {
            record: slot.record,
            subject: *subject,
            predicate,
            object: *object,
            properties,
            envelope: rec.envelope(),
            state: self.state_at(slot.record)?,
            view: *self,
        })
    }

    /// Conflicts live *at this instant*: promoted claims overlapping now. A
    /// subset of `ledger.contradictions()`, which flags overlap anywhere on
    /// the valid-time line.
    pub fn conflicts(&self) -> Vec<(EntityId, &'a str, Property<'a>)> {
        let mut found = Vec::new();
        for ((entity, name), slots) in &self.projection.attributes {
            let admitted: Vec<&Slot> = slots.iter().filter(|s| self.admits(s)).collect();
            if admitted.len() >= 2 {
                let property = self.property_from(&admitted);
                if let Some(p) = property {
                    found.push((*entity, name.as_str(), p));
                }
            }
        }
        found
    }

    fn property_from(&self, slots: &[&Slot]) -> Option<Property<'a>> {
        let mut claims: Vec<PropertyClaim<'a>> = Vec::new();
        for slot in slots {
            let rec = self.ledger.record(slot.record)?;
            let Content::Claim(ClaimContent::Attribute { value, .. }) = rec.content() else {
                continue;
            };
            claims.push(PropertyClaim {
                record: slot.record,
                seq: slot.seq,
                value,
                envelope: rec.envelope(),
                state: self.state_at(slot.record)?,
            });
        }
        claims.sort_by_key(|c| c.seq);
        match claims.len() {
            0 => None,
            1 => Some(Property::Single(claims.remove(0))),
            _ => Some(Property::Conflicted(claims)),
        }
    }

    /// A gap has no claim state, so `admits` cannot judge it — but every other
    /// predicate still applies. Dropping the whole of `admits` here once meant
    /// answered and withdrawn gaps stayed in the graph forever and the author
    /// filter silently did not apply to them.
    fn admits_gap(&self, slot: &Slot) -> bool {
        if slot.recorded_at > self.spec.record_time {
            return false;
        }
        if let Some(kind) = self.spec.author_kind
            && slot.author_kind != kind
        {
            return false;
        }
        if !slot.validity.contains(self.spec.valid_at) {
            return false;
        }
        match self.ledger.state_of_at(slot.record, self.spec.record_time) {
            // A registered gap is a live open question; answered and withdrawn
            // ones are history, and only the forensic view keeps them.
            Some(RecordState::Gap(GapState::Registered)) => true,
            Some(RecordState::Gap(_)) => self.spec.states == StateFilter::All,
            _ => false,
        }
    }

    /// Prose and gap records referencing this entity, in log order.
    pub fn about(&self, entity: EntityId) -> Vec<&'a Record> {
        self.projection
            .about
            .get(&entity)
            .into_iter()
            .flatten()
            .filter(|slot| {
                let Some(rec) = self.ledger.record(slot.record) else { return false };
                match rec.content() {
                    Content::Gap(_) => self.admits_gap(slot),
                    _ => self.admits(slot),
                }
            })
            .filter_map(|slot| self.ledger.record(slot.record))
            .collect()
    }

    // ── Weighted paths (R-5) ────────────────────────────────────────────────

    /// Lowest-cost path between two entities, with edge costs drawn from the
    /// instrument panel. Costs come from measurements, never from the governed
    /// ledger, so the graph can learn its own weights without a verdict.
    pub fn shortest_path(
        &self,
        from: EntityId,
        to: EntityId,
        cost: &CostSpec,
    ) -> Result<Option<Path>, Error> {
        if self.ledger.entity(from).is_none() {
            return Err(Error::UnknownEntity(from));
        }
        if self.ledger.entity(to).is_none() {
            return Err(Error::UnknownEntity(to));
        }
        if from == to {
            return Ok(Some(Path { edges: Vec::new(), total_cost: 0.0 }));
        }

        let mut best: BTreeMap<EntityId, f64> = BTreeMap::new();
        let mut came_from: BTreeMap<EntityId, (EntityId, RecordId)> = BTreeMap::new();
        let mut heap: BinaryHeap<Reverse<(Cost, EntityId)>> = BinaryHeap::new();
        best.insert(from, 0.0);
        heap.push(Reverse((Cost(0.0), from)));

        while let Some(Reverse((Cost(spent), node))) = heap.pop() {
            if node == to {
                break;
            }
            if best.get(&node).is_some_and(|b| spent > *b) {
                continue;
            }
            for edge in self.out_slots(node) {
                let Some(step) = self.edge_cost(edge.slot.record, cost)? else { continue };
                let next = spent + step;
                if best.get(&edge.other).is_none_or(|b| next < *b) {
                    best.insert(edge.other, next);
                    came_from.insert(edge.other, (node, edge.slot.record));
                    heap.push(Reverse((Cost(next), edge.other)));
                }
            }
        }

        let Some(total_cost) = best.get(&to).copied() else { return Ok(None) };
        let mut edges = Vec::new();
        let mut cursor = to;
        while cursor != from {
            let (previous, record) = came_from[&cursor];
            edges.push(record);
            cursor = previous;
        }
        edges.reverse();
        Ok(Some(Path { edges, total_cost }))
    }

    fn out_slots(&self, node: EntityId) -> Vec<EdgeSlot> {
        self.projection
            .out_edges
            .get(&node)
            .into_iter()
            .flatten()
            .filter(|e| self.admits(&e.slot))
            .copied()
            .collect()
    }

    fn edge_cost(&self, record: RecordId, spec: &CostSpec) -> Result<Option<f64>, Error> {
        let measured = self
            .ledger
            .measurement(MeasurementTarget::Relation(record), &spec.measurement)
            .map(|m| m.value);
        let raw = match measured {
            Some(v) => v,
            None => match spec.missing {
                MissingCost::Exclude => return Ok(None),
                MissingCost::Default(d) => return validate_cost(d, record).map(Some),
            },
        };
        validate_cost(spec.transform.apply(raw), record).map(Some)
    }
}

fn validate_cost(cost: f64, record: RecordId) -> Result<f64, Error> {
    if cost.is_finite() && cost >= 0.0 {
        Ok(cost)
    } else {
        Err(Error::InvalidCost { record, cost })
    }
}

/// How a raw measurement becomes a traversal cost.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CostTransform {
    /// Use the measurement as the cost.
    Identity,
    /// `-ln(p)` — turns a success probability into an additive cost, so the
    /// cheapest path is the most likely one.
    NegLn,
}

impl CostTransform {
    fn apply(self, raw: f64) -> f64 {
        match self {
            CostTransform::Identity => raw,
            CostTransform::NegLn => -raw.ln(),
        }
    }
}

/// What to do with an edge that has no such measurement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MissingCost {
    /// The edge is not traversable.
    Exclude,
    /// Traverse at this cost.
    Default(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CostSpec {
    pub measurement: String,
    pub transform: CostTransform,
    pub missing: MissingCost,
}

impl CostSpec {
    /// Every admitted edge costs 1 — hop count.
    pub fn hops() -> Self {
        Self {
            measurement: String::new(),
            transform: CostTransform::Identity,
            missing: MissingCost::Default(1.0),
        }
    }

    pub fn measured(name: impl Into<String>, transform: CostTransform) -> Self {
        Self { measurement: name.into(), transform, missing: MissingCost::Exclude }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    pub edges: Vec<RecordId>,
    pub total_cost: f64,
}

/// Total-ordered f64 for the heap. Costs are validated finite and
/// non-negative before they get here.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Cost(f64);

impl Eq for Cost {}

impl Ord for Cost {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl PartialOrd for Cost {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

// ── Projected elements ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct Node<'a> {
    view: GraphView<'a>,
    entity: &'a Entity,
}

impl<'a> Node<'a> {
    pub fn id(&self) -> EntityId {
        self.entity.id()
    }

    pub fn kind(&self) -> &'a str {
        self.entity.kind()
    }

    pub fn label(&self) -> &'a str {
        self.entity.label()
    }

    pub fn property(&self, name: &str) -> Option<Property<'a>> {
        let slots = self.view.projection.attributes.get(&(self.entity.id(), name.to_string()))?;
        let admitted: Vec<&Slot> = slots.iter().filter(|s| self.view.admits(s)).collect();
        self.view.property_from(&admitted)
    }

    pub fn properties(&self) -> BTreeMap<&'a str, Property<'a>> {
        let mut out = BTreeMap::new();
        for ((entity, name), slots) in &self.view.projection.attributes {
            if *entity != self.entity.id() {
                continue;
            }
            let admitted: Vec<&Slot> = slots.iter().filter(|s| self.view.admits(s)).collect();
            if let Some(property) = self.view.property_from(&admitted) {
                out.insert(name.as_str(), property);
            }
        }
        out
    }

    pub fn out_edges(&self) -> Vec<Edge<'a>> {
        let mut edges: Vec<Edge<'a>> = self
            .view
            .out_slots(self.entity.id())
            .iter()
            .filter_map(|e| self.view.edge_from_slot(&e.slot))
            .collect();
        edges.sort_by_key(|e| e.seq());
        edges
    }

    pub fn in_edges(&self) -> Vec<Edge<'a>> {
        let mut edges: Vec<Edge<'a>> = self
            .view
            .projection
            .in_edges
            .get(&self.entity.id())
            .into_iter()
            .flatten()
            .filter(|e| self.view.admits(&e.slot))
            .filter_map(|e| self.view.edge_from_slot(&e.slot))
            .collect();
        edges.sort_by_key(|e| e.seq());
        edges
    }

    /// Prose claims and gaps that name this entity.
    pub fn about(&self) -> Vec<&'a Record> {
        self.view.about(self.entity.id())
    }

    pub fn measurements(&self) -> Vec<&'a Measurement> {
        self.view.ledger.measurements_for(MeasurementTarget::Entity(self.entity.id()))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Edge<'a> {
    record: RecordId,
    subject: EntityId,
    predicate: &'a str,
    object: EntityId,
    properties: &'a BTreeMap<String, Value>,
    envelope: &'a Envelope,
    state: ClaimState,
    view: GraphView<'a>,
}

impl<'a> Edge<'a> {
    pub fn record(&self) -> RecordId {
        self.record
    }

    pub fn subject(&self) -> EntityId {
        self.subject
    }

    pub fn predicate(&self) -> &'a str {
        self.predicate
    }

    pub fn object(&self) -> EntityId {
        self.object
    }

    pub fn properties(&self) -> &'a BTreeMap<String, Value> {
        self.properties
    }

    pub fn envelope(&self) -> &'a Envelope {
        self.envelope
    }

    /// Every projected element reports its own state, so a non-default view
    /// labels rather than lies.
    pub fn state(&self) -> ClaimState {
        self.state
    }

    pub fn measurements(&self) -> Vec<&'a Measurement> {
        self.view.ledger.measurements_for(MeasurementTarget::Relation(self.record))
    }

    fn seq(&self) -> u64 {
        self.view
            .projection
            .out_edges
            .get(&self.subject)
            .into_iter()
            .flatten()
            .find(|e| e.slot.record == self.record)
            .map(|e| e.slot.seq)
            .unwrap_or(u64::MAX)
    }
}

/// One admitted attribute claim.
#[derive(Debug, Clone, Copy)]
pub struct PropertyClaim<'a> {
    record: RecordId,
    seq: u64,
    value: &'a Value,
    envelope: &'a Envelope,
    state: ClaimState,
}

impl<'a> PropertyClaim<'a> {
    pub fn record(&self) -> RecordId {
        self.record
    }

    pub fn value(&self) -> &'a Value {
        self.value
    }

    pub fn envelope(&self) -> &'a Envelope {
        self.envelope
    }

    pub fn state(&self) -> ClaimState {
        self.state
    }
}

/// A node property. There is deliberately no `value()` accessor: nothing here
/// returns one value without the caller having visibly decided what to do
/// about a conflict, because silently picking a winner is exactly what
/// invariant 7 exists to prevent.
#[derive(Debug, Clone)]
pub enum Property<'a> {
    Single(PropertyClaim<'a>),
    /// Two or more admitted claims, in log order. Length is always >= 2.
    Conflicted(Vec<PropertyClaim<'a>>),
}

impl<'a> Property<'a> {
    /// The claim, when there is exactly one.
    pub fn single(&self) -> Option<&PropertyClaim<'a>> {
        match self {
            Property::Single(claim) => Some(claim),
            Property::Conflicted(_) => None,
        }
    }

    pub fn claims(&self) -> &[PropertyClaim<'a>] {
        match self {
            Property::Single(claim) => std::slice::from_ref(claim),
            Property::Conflicted(claims) => claims,
        }
    }

    pub fn is_conflicted(&self) -> bool {
        matches!(self, Property::Conflicted(_))
    }

    /// The value when every admitted claim agrees — the honest common case of
    /// two records with different provenance saying the same thing. Note that
    /// they stay two records: agreement is not dedup (U-12).
    pub fn unanimous_value(&self) -> Option<&'a Value> {
        let claims = self.claims();
        let first = claims.first()?.value;
        claims.iter().all(|c| c.value == first).then_some(first)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{GapContent, RetireReason, VerdictAction, VerdictContent};
    use crate::envelope::{Author, SourceRef};
    use crate::record::Draft;

    fn ts(offset: i64) -> Timestamp {
        Timestamp::from_second(1_756_000_000 + offset).unwrap()
    }

    struct Fixture {
        ledger: MemoryLedger,
        a: EntityId,
        b: EntityId,
        c: EntityId,
    }

    fn fixture() -> Fixture {
        let mut ledger = MemoryLedger::new();
        let a = ledger.add_entity("station", "A");
        let b = ledger.add_entity("station", "B");
        let c = ledger.add_entity("station", "C");
        Fixture { ledger, a, b, c }
    }

    fn attribute(subject: EntityId, name: &str, value: f64, author: Author) -> Draft {
        Draft::new(
            author,
            SourceRef::channel("test"),
            Content::Claim(ClaimContent::Attribute {
                subject,
                name: name.into(),
                value: Value::Number(value),
            }),
        )
    }

    fn relation(subject: EntityId, predicate: &str, object: EntityId) -> Draft {
        Draft::new(
            Author::human("Greg"),
            SourceRef::channel("test"),
            Content::Claim(ClaimContent::Relation {
                subject,
                predicate: predicate.into(),
                object,
                properties: BTreeMap::new(),
            }),
        )
    }

    fn promote(target: RecordId) -> Draft {
        Draft::new(
            Author::human("Greg"),
            SourceRef::channel("huddle"),
            Content::Verdict(VerdictContent {
                action: VerdictAction::Promote { target, retiring: None },
                rationale: None,
            }),
        )
    }

    #[test]
    fn only_promoted_claims_reach_the_default_view() {
        let mut f = fixture();
        let promoted = f.ledger.append(attribute(f.a, "torque", 24.0, Author::human("G"))).unwrap();
        f.ledger.append(promote(promoted)).unwrap();
        f.ledger.append(attribute(f.a, "speed", 5.0, Author::agent("miner"))).unwrap();

        let projection = Projection::rebuild(&f.ledger);
        let view = projection.view(&f.ledger, ViewSpec::now());
        let node = view.node(f.a).unwrap();
        assert!(node.property("torque").is_some());
        assert!(node.property("speed").is_none());

        let with_proposed = projection
            .view(&f.ledger, ViewSpec::now().with_states(StateFilter::PromotedAndProposed));
        let node = with_proposed.node(f.a).unwrap();
        let speed = node.property("speed").unwrap();
        // A non-default view labels rather than lies.
        assert_eq!(speed.single().unwrap().state(), ClaimState::Proposed);
    }

    #[test]
    fn retirement_removes_from_the_view_without_removing_from_the_index() {
        let mut f = fixture();
        let claim = f.ledger.append(attribute(f.a, "torque", 24.0, Author::human("G"))).unwrap();
        f.ledger.append(promote(claim)).unwrap();
        let mut projection = Projection::rebuild(&f.ledger);
        assert!(projection.view(&f.ledger, ViewSpec::now()).node(f.a).unwrap().property("torque").is_some());

        f.ledger
            .append(Draft::new(
                Author::human("Greg"),
                SourceRef::channel("huddle"),
                Content::Verdict(VerdictContent {
                    action: VerdictAction::Retire {
                        target: claim,
                        reason: RetireReason::NoLongerTrue,
                    },
                    rationale: None,
                }),
            ))
            .unwrap();
        projection.advance(&f.ledger);

        assert!(projection.view(&f.ledger, ViewSpec::now()).node(f.a).unwrap().property("torque").is_none());
        // The slot is still indexed — the forensic view still finds it.
        let all = projection.view(&f.ledger, ViewSpec::now().with_states(StateFilter::All));
        assert_eq!(
            all.node(f.a).unwrap().property("torque").unwrap().single().unwrap().state(),
            ClaimState::Retired
        );
    }

    #[test]
    fn valid_time_is_a_read_parameter_not_index_state() {
        let mut f = fixture();
        let mut draft = attribute(f.a, "torque", 24.0, Author::human("G"));
        draft.valid_from = Some(ts(100));
        draft.valid_to = Some(ts(200));
        let claim = f.ledger.append(draft).unwrap();
        f.ledger.append(promote(claim)).unwrap();

        let projection = Projection::rebuild(&f.ledger);
        let before = projection.view(&f.ledger, ViewSpec::bitemporal(Timestamp::now(), ts(50)));
        let during = projection.view(&f.ledger, ViewSpec::bitemporal(Timestamp::now(), ts(150)));
        let after = projection.view(&f.ledger, ViewSpec::bitemporal(Timestamp::now(), ts(200)));

        assert!(before.node(f.a).unwrap().property("torque").is_none());
        assert!(during.node(f.a).unwrap().property("torque").is_some());
        // Half-open [from, to): the end instant is already outside.
        assert!(after.node(f.a).unwrap().property("torque").is_none());
    }

    #[test]
    fn conflicts_surface_and_cannot_be_silently_resolved() {
        let mut f = fixture();
        let a = f.ledger.append(attribute(f.a, "torque", 24.0, Author::human("G"))).unwrap();
        f.ledger.append(promote(a)).unwrap();
        let b = f.ledger.append(attribute(f.a, "torque", 26.0, Author::human("M"))).unwrap();
        f.ledger.append(promote(b)).unwrap();

        let projection = Projection::rebuild(&f.ledger);
        let view = projection.view(&f.ledger, ViewSpec::now());
        let property = view.node(f.a).unwrap().property("torque").unwrap();
        assert!(property.is_conflicted());
        assert!(property.single().is_none());
        assert_eq!(property.claims().len(), 2);
        assert_eq!(property.unanimous_value(), None);
        assert_eq!(view.conflicts().len(), 1);
    }

    #[test]
    fn agreeing_claims_stay_two_records_but_read_as_unanimous() {
        let mut f = fixture();
        let a = f.ledger.append(attribute(f.a, "torque", 24.0, Author::human("G"))).unwrap();
        f.ledger.append(promote(a)).unwrap();
        let b = f.ledger.append(attribute(f.a, "torque", 24.0, Author::human("M"))).unwrap();
        f.ledger.append(promote(b)).unwrap();

        let projection = Projection::rebuild(&f.ledger);
        let property = projection
            .view(&f.ledger, ViewSpec::now())
            .node(f.a)
            .unwrap()
            .property("torque")
            .unwrap();
        assert!(property.is_conflicted());
        assert_eq!(property.unanimous_value(), Some(&Value::Number(24.0)));
    }

    #[test]
    fn parallel_relation_claims_are_two_edges() {
        let mut f = fixture();
        let first = f.ledger.append(relation(f.a, "feeds", f.b)).unwrap();
        f.ledger.append(promote(first)).unwrap();
        let second = f.ledger.append(relation(f.a, "feeds", f.b)).unwrap();
        f.ledger.append(promote(second)).unwrap();

        let projection = Projection::rebuild(&f.ledger);
        let view = projection.view(&f.ledger, ViewSpec::now());
        assert_eq!(view.node(f.a).unwrap().out_edges().len(), 2);
        assert_eq!(view.node(f.b).unwrap().in_edges().len(), 2);
    }

    #[test]
    fn prose_claims_are_reachable_through_about() {
        let mut f = fixture();
        let claim = f
            .ledger
            .append(Draft::new(
                Author::human("Greg"),
                SourceRef::channel("interview"),
                Content::Claim(ClaimContent::Text {
                    body: "the station runs hot in summer".into(),
                    about: vec![f.a],
                }),
            ))
            .unwrap();
        f.ledger.append(promote(claim)).unwrap();
        f.ledger
            .append(Draft::new(
                Author::agent("assistant"),
                SourceRef::channel("chat"),
                Content::Gap(GapContent {
                    question: "how hot?".into(),
                    territory: vec![f.a],
                }),
            ))
            .unwrap();

        let projection = Projection::rebuild(&f.ledger);
        let view = projection.view(&f.ledger, ViewSpec::now());
        // One promoted prose claim plus one registered gap.
        assert_eq!(view.node(f.a).unwrap().about().len(), 2);
        assert!(view.node(f.b).unwrap().about().is_empty());
    }

    /// Regression: the gap arm of `about()` once bypassed every filter, so a
    /// closed gap stayed in the graph forever.
    #[test]
    fn closed_gaps_leave_the_graph() {
        let mut f = fixture();
        let gap = f
            .ledger
            .append(Draft::new(
                Author::agent("assistant"),
                SourceRef::channel("chat"),
                Content::Gap(GapContent { question: "how hot?".into(), territory: vec![f.a] }),
            ))
            .unwrap();
        let projection = Projection::rebuild(&f.ledger);
        assert_eq!(projection.view(&f.ledger, ViewSpec::now()).node(f.a).unwrap().about().len(), 1);

        f.ledger
            .append(Draft::new(
                Author::human("Greg"),
                SourceRef::channel("huddle"),
                Content::Verdict(VerdictContent {
                    action: VerdictAction::Withdraw { gap },
                    rationale: None,
                }),
            ))
            .unwrap();
        let projection = Projection::rebuild(&f.ledger);
        assert!(projection.view(&f.ledger, ViewSpec::now()).node(f.a).unwrap().about().is_empty());
        // The forensic view still has it.
        let all = projection.view(&f.ledger, ViewSpec::now().with_states(StateFilter::All));
        assert_eq!(all.node(f.a).unwrap().about().len(), 1);
    }

    /// Regression: the author filter applied to prose claims but silently not
    /// to gaps, so "only what humans recorded" returned agent-authored gaps.
    #[test]
    fn the_author_filter_applies_to_gaps_too() {
        let mut f = fixture();
        let prose = f
            .ledger
            .append(Draft::new(
                Author::agent("assistant"),
                SourceRef::channel("chat"),
                Content::Claim(ClaimContent::Text { body: "a note".into(), about: vec![f.a] }),
            ))
            .unwrap();
        f.ledger.append(promote(prose)).unwrap();
        f.ledger
            .append(Draft::new(
                Author::agent("assistant"),
                SourceRef::channel("chat"),
                Content::Gap(GapContent { question: "how hot?".into(), territory: vec![f.a] }),
            ))
            .unwrap();

        let projection = Projection::rebuild(&f.ledger);
        let all = projection.view(&f.ledger, ViewSpec::now());
        assert_eq!(all.node(f.a).unwrap().about().len(), 2);

        let humans = projection.view(&f.ledger, ViewSpec::now().by_author_kind(AuthorKind::Human));
        assert!(
            humans.node(f.a).unwrap().about().is_empty(),
            "everything here was agent-authored"
        );
        let agents = projection.view(&f.ledger, ViewSpec::now().by_author_kind(AuthorKind::Agent));
        assert_eq!(agents.node(f.a).unwrap().about().len(), 2);
    }

    #[test]
    fn dark_entities_are_still_nodes() {
        let f = fixture();
        let projection = Projection::rebuild(&f.ledger);
        let view = projection.view(&f.ledger, ViewSpec::now());
        assert_eq!(view.nodes().len(), 3);
        assert!(view.node(f.c).unwrap().properties().is_empty());
    }

    #[test]
    fn weighted_paths_follow_the_instrument_panel() {
        let mut f = fixture();
        // A -> B -> C, plus a direct A -> C that is likelier to fail.
        let ab = f.ledger.append(relation(f.a, "joins", f.b)).unwrap();
        f.ledger.append(promote(ab)).unwrap();
        let bc = f.ledger.append(relation(f.b, "joins", f.c)).unwrap();
        f.ledger.append(promote(bc)).unwrap();
        let ac = f.ledger.append(relation(f.a, "joins", f.c)).unwrap();
        f.ledger.append(promote(ac)).unwrap();

        let updater = Author::agent("join-updater");
        for (edge, rate) in [(ab, 0.99), (bc, 0.99), (ac, 0.5)] {
            f.ledger
                .record_measurement(
                    MeasurementTarget::Relation(edge),
                    "success_rate",
                    rate,
                    updater.clone(),
                    ts(0),
                )
                .unwrap();
        }

        let projection = Projection::rebuild(&f.ledger);
        let view = projection.view(&f.ledger, ViewSpec::now());
        let spec = CostSpec::measured("success_rate", CostTransform::NegLn);
        let path = view.shortest_path(f.a, f.c, &spec).unwrap().unwrap();
        assert_eq!(path.edges, vec![ab, bc]);

        // Hop count prefers the direct edge — same graph, different cost model.
        let hops = view.shortest_path(f.a, f.c, &CostSpec::hops()).unwrap().unwrap();
        assert_eq!(hops.edges, vec![ac]);

        // The graph learns: raise the direct edge's success rate and the
        // weighted answer changes with no verdict and no rebuild.
        f.ledger
            .record_measurement(
                MeasurementTarget::Relation(ac),
                "success_rate",
                0.999,
                updater,
                ts(1),
            )
            .unwrap();
        let view = projection.view(&f.ledger, ViewSpec::now());
        let path = view.shortest_path(f.a, f.c, &spec).unwrap().unwrap();
        assert_eq!(path.edges, vec![ac]);
    }

    #[test]
    fn unmeasured_edges_are_excluded_or_defaulted() {
        let mut f = fixture();
        let ab = f.ledger.append(relation(f.a, "joins", f.b)).unwrap();
        f.ledger.append(promote(ab)).unwrap();

        let projection = Projection::rebuild(&f.ledger);
        let view = projection.view(&f.ledger, ViewSpec::now());
        let excluded = CostSpec::measured("success_rate", CostTransform::NegLn);
        assert_eq!(view.shortest_path(f.a, f.b, &excluded).unwrap(), None);
        assert!(view.shortest_path(f.a, f.b, &CostSpec::hops()).unwrap().is_some());
    }

    #[test]
    fn negative_costs_are_an_error_not_a_wrong_answer() {
        let mut f = fixture();
        let ab = f.ledger.append(relation(f.a, "joins", f.b)).unwrap();
        f.ledger.append(promote(ab)).unwrap();
        f.ledger
            .record_measurement(
                MeasurementTarget::Relation(ab),
                "success_rate",
                2.0, // -ln(2) < 0
                Author::agent("updater"),
                ts(0),
            )
            .unwrap();

        let projection = Projection::rebuild(&f.ledger);
        let view = projection.view(&f.ledger, ViewSpec::now());
        let spec = CostSpec::measured("success_rate", CostTransform::NegLn);
        assert!(matches!(
            view.shortest_path(f.a, f.b, &spec),
            Err(Error::InvalidCost { .. })
        ));
    }

    #[test]
    fn record_time_views_see_the_past_projection() {
        let mut f = fixture();
        let claim = f
            .ledger
            .append_at(attribute(f.a, "torque", 24.0, Author::human("G")), ts(10))
            .unwrap();
        f.ledger.append_at(promote(claim), ts(30)).unwrap();

        let projection = Projection::rebuild(&f.ledger);
        // Before promotion: the claim exists but is not promoted.
        let early = projection.view(&f.ledger, ViewSpec::at(ts(20)));
        assert!(early.node(f.a).unwrap().property("torque").is_none());
        // After: it is in the graph.
        let late = projection.view(&f.ledger, ViewSpec::at(ts(40)));
        assert!(late.node(f.a).unwrap().property("torque").is_some());
        // Before the claim was even recorded.
        let earliest = projection.view(&f.ledger, ViewSpec::at(ts(5)));
        assert!(earliest.node(f.a).unwrap().property("torque").is_none());
    }

    /// U-10, the definitional half: `rebuild` is `empty().advance()`, so an
    /// interleaved fold and a single end-to-end fold must agree.
    #[test]
    fn incremental_advance_equals_rebuild() {
        let mut f = fixture();
        let mut incremental = Projection::empty();

        let a1 = f.ledger.append(attribute(f.a, "torque", 24.0, Author::human("G"))).unwrap();
        incremental.advance(&f.ledger);
        f.ledger.append(promote(a1)).unwrap();
        incremental.advance(&f.ledger);
        let e1 = f.ledger.append(relation(f.a, "feeds", f.b)).unwrap();
        incremental.advance(&f.ledger);
        f.ledger.append(promote(e1)).unwrap();
        let a2 = f.ledger.append(attribute(f.b, "torque", 26.0, Author::agent("m"))).unwrap();
        incremental.advance(&f.ledger);
        f.ledger
            .append(Draft::new(
                Author::human("Greg"),
                SourceRef::channel("huddle"),
                Content::Verdict(VerdictContent {
                    action: VerdictAction::Reject { target: a2 },
                    rationale: None,
                }),
            ))
            .unwrap();
        incremental.advance(&f.ledger);

        assert_eq!(incremental, Projection::rebuild(&f.ledger));
        // And a redundant advance changes nothing.
        let consumed = incremental.advance(&f.ledger);
        assert_eq!(consumed, 0);
        assert_eq!(incremental, Projection::rebuild(&f.ledger));
    }

    /// The index must agree with the ledger about state for every claim,
    /// whatever its content shape — including prose claims, which carry no
    /// attribute or relation slot.
    #[test]
    fn index_state_agrees_with_ledger_for_every_claim_shape() {
        let mut f = fixture();
        let prose = f
            .ledger
            .append(Draft::new(
                Author::human("Greg"),
                SourceRef::channel("interview"),
                Content::Claim(ClaimContent::Text { body: "a note".into(), about: vec![] }),
            ))
            .unwrap();
        f.ledger.append(promote(prose)).unwrap();
        let pattern = f
            .ledger
            .append(Draft::new(
                Author::human("Greg"),
                SourceRef::channel("interview"),
                Content::Claim(ClaimContent::Pattern {
                    context: "c".into(),
                    forces: vec!["f".into()],
                    solution: "s".into(),
                    about: vec![f.a],
                }),
            ))
            .unwrap();

        let projection = Projection::rebuild(&f.ledger);
        for record in f.ledger.records() {
            if let Some(RecordState::Claim(expected)) = f.ledger.state_of(record.id()) {
                assert_eq!(
                    projection.state_of(record.id()),
                    Some(expected),
                    "index and ledger disagree for {}",
                    record.id()
                );
            }
        }
        assert_eq!(projection.state_of(prose), Some(ClaimState::Promoted));
        assert_eq!(projection.state_of(pattern), Some(ClaimState::Proposed));
    }
}

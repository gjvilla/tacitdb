use crate::envelope::Author;
use crate::id::{EntityId, RecordId};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum MeasurementTarget {
    Entity(EntityId),
    /// A projected edge — the relation claim it came from.
    Relation(RecordId),
}

/// The instrument panel's unit (design/001 §1.3): machine-owned, mutable in
/// place, no envelope, no verdicts — and never an answer to "what does the
/// organization know" (invariant 8).
#[derive(Debug, Clone, Serialize)]
pub struct Measurement {
    pub target: MeasurementTarget,
    pub name: String,
    pub value: f64,
    pub updated_at: Timestamp,
    pub updated_by: Author,
}

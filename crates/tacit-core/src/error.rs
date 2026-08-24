use crate::content::RecordKind;
use crate::id::{EntityId, RecordId};
use crate::state::RecordState;
use jiff::Timestamp;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unknown entity {0}")]
    UnknownEntity(EntityId),

    #[error("unknown record {0}")]
    UnknownRecord(RecordId),

    #[error("entity {0} is referenced twice by the same record")]
    DuplicateEntityRef(EntityId),

    #[error("evidence must reference an entity of kind \"source\"; {0} has kind \"{1}\"")]
    EvidenceNotSource(EntityId, String),

    #[error("verdicts require a human-declared author (invariants 5 and 6)")]
    VerdictRequiresHumanAuthor,

    #[error("{action} expects a {expected}, but {target} is a {actual}")]
    WrongTargetKind {
        action: &'static str,
        target: RecordId,
        expected: RecordKind,
        actual: RecordKind,
    },

    #[error("illegal transition: cannot {action} {target} in state {state}")]
    IllegalTransition {
        action: &'static str,
        target: RecordId,
        state: RecordState,
    },

    #[error("a gap is answered by a promoted claim; {claim} is in state {state}")]
    AnswerRequiresPromotedClaim { claim: RecordId, state: RecordState },

    #[error("a promotion cannot retire its own target")]
    PromoteRetireSameRecord,

    #[error("measurement target {0} is not a relation claim")]
    MeasurementTargetNotRelation(RecordId),

    #[error("valid_to must be after valid_from")]
    InvalidValidity,

    /// The log must be a prefix of time: `state_of_at` filters verdicts by
    /// `recorded_at` while walking the log in order, so a backwards step would
    /// let it report a state the ledger never held.
    #[error("record-time must not move backwards: {proposed} precedes the last recorded {last}")]
    NonMonotonicRecordTime { proposed: Timestamp, last: Timestamp },

    #[error("record-time {proposed} is in the future (now {now})")]
    FutureRecordTime { proposed: Timestamp, now: Timestamp },

    /// Dijkstra requires finite, non-negative costs; a cost transform that
    /// produces otherwise is a modelling error, not a traversal to guess at.
    #[error("edge {record} has invalid traversal cost {cost}")]
    InvalidCost { record: RecordId, cost: f64 },

    #[error("storage error at {path}: {detail}")]
    Storage { path: std::path::PathBuf, detail: String },

    #[error("{path} is corrupt at line {line}: {detail}")]
    CorruptJournal { path: std::path::PathBuf, line: usize, detail: String },

    #[error("envelope version {found} is not supported (this build reads {supported})")]
    UnsupportedEnvelopeVersion { found: u16, supported: u16 },
}

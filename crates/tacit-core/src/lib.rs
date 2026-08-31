//! Tacit core: the governed ledger, the instrument panel, the projected graph,
//! and the grammar of the write-path ratchet — the engine invariants of
//! `docs/design/001-data-model.md` expressed as types and append-time checks.
//!
//! What the types themselves guarantee: records carry no state field (state is
//! a fold over verdicts), envelopes and ids cannot be minted outside the
//! ledger, the store exposes no mutation or deletion, measurements are a
//! separate type in a separate store, and the projected graph is a caller-held
//! view that the write path cannot reference.

mod content;
mod embedding;
mod entity;
mod envelope;
mod error;
mod id;
mod journal;
#[cfg(test)]
mod journal_tests;
mod ledger;
mod measurement;
mod projection;
#[cfg(test)]
mod proptests;
mod record;
mod redact;
mod retrieval;
mod state;
mod validity;
mod value;

pub use content::{
    REDACTED, RedactionContent, RedactionScope,
    ClaimContent, Content, GapContent, HypothesisContent, RecordKind, RetireReason, ScoreOutcome, SetBasis,
    VerdictAction, VerdictContent, WithdrawReason,
};
pub use embedding::{
    Embedded, Embedder, HashingEmbedder, Neighbourhoods, VectorIndex, similarity,
};
pub use entity::Entity;
pub use envelope::{
    RedactionMark,
    Author, AuthorKind, ENVELOPE_VERSION, Envelope, Evidence, ReviewTrigger, SourceRef,
};
pub use error::Error;
pub use id::{EntityId, IdParseError, RecordId};
pub use journal::{Event, Recovery};
pub use ledger::{
    Contradiction, Ledger, Opened, Pending, Ratification, ReviewQueue, SOURCE_KIND,
};
pub use measurement::{Measurement, MeasurementTarget};
pub use projection::{
    CostSpec, CostTransform, Edge, GraphView, MissingCost, Node, Path, Projection, Property,
    PropertyClaim, StateFilter, ViewSpec,
};
pub use record::{Draft, Record};
pub use redact::{RedactReport, redact_store};
pub use retrieval::{
    BeyondView, Budget, Direction, Expansion, Fusion, Item, Outcome, Probe, Query, Ranking, Retrieved, Retriever,
    TextIndex, Via, fuse, indexable_text, tokenize,
};
pub use state::{ClaimState, GapState, HypothesisState, RecordState};
pub use validity::Validity;
pub use value::Value;

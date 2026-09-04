//! Tacit core: the governed ledger, the instrument panel, the projected graph,
//! and the grammar of the write-path ratchet — the engine invariants of
//! `docs/design/001-data-model.md` expressed as types and append-time checks.
//!
//! What the types themselves guarantee: records carry no state field (state is
//! a fold over verdicts), envelopes and ids cannot be minted outside the
//! ledger, the store exposes no mutation or deletion, measurements are a
//! separate type in a separate store, and the projected graph is a caller-held
//! view that the write path cannot reference.
//!
//! The whole loop in one place — an agent proposes, a person rules, a question
//! is asked and answered or honestly declined:
//!
//! ```
//! use tacit_core::{
//!     Author, ClaimContent, Content, Draft, Ledger, Outcome, Projection, Query, SourceRef,
//!     TextIndex, VerdictAction, VerdictContent, ViewSpec,
//! };
//!
//! # fn main() -> Result<(), tacit_core::Error> {
//! let mut ledger = Ledger::new();
//! let billing = ledger.add_entity("service", "billing")?;
//!
//! // An agent proposes. It lands as `proposed` and stays there.
//! let claim = ledger.append(Draft::new(
//!     Author::agent("cost-bot"),
//!     SourceRef::channel("nightly report"),
//!     Content::Claim(ClaimContent::Text {
//!         body: "billing runs on the shared Postgres cluster".into(),
//!         about: vec![billing],
//!     }),
//! ))?;
//!
//! // A person rules. A verdict is a record too, and only a human may author one.
//! ledger.append(Draft::new(
//!     Author::human("Jordan Lee"),
//!     SourceRef::channel("keyboard"),
//!     Content::Verdict(VerdictContent {
//!         action: VerdictAction::Promote { target: claim, retiring: None },
//!         rationale: Some("confirmed in the runbook".into()),
//!     }),
//! ))?;
//!
//! // Ask. The default view is what the organization has actually agreed.
//! let index = TextIndex::rebuild(&ledger);
//! let projection = Projection::rebuild(&ledger);
//! let retriever = index.retriever(&ledger, &projection, ViewSpec::now());
//!
//! let found = retriever.retrieve(&Query::text("where does billing run"));
//! assert_eq!(found.outcome, Outcome::Matches);
//! assert_eq!(found.items[0].record.id(), claim);
//!
//! // A question the record cannot settle is declined, and the words it has
//! // never written are named.
//! let miss = retriever.retrieve(&Query::text("who owns the mobile app"));
//! assert!(miss.is_abstention());
//! assert!(miss.unknown_terms.contains(&"mobile".to_string()));
//! # Ok(())
//! # }
//! ```

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
    REDACTED, RedactionContent, RedactionScope, RedactionTarget,
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
    TextIndex, Via, fuse, indexable_text, tokenize, TitleFold, DEFAULT_STOPWORDS,
};
pub use state::{ClaimState, GapState, HypothesisState, RecordState};
pub use validity::Validity;
pub use value::Value;

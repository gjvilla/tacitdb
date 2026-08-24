//! The keeper layer: content, files, and corpus judgment.
//!
//! Everything that knows about markdown, the filesystem, or what a decision
//! record *means* lives here rather than in `tacit-core`, which owns only the
//! grammar (D-0002's two-layer bet, made a crate boundary).

pub mod corpus;
pub mod golden;
pub mod parse;
pub mod register;

pub use corpus::{
    DECISION_KIND, IngestError, IngestReport, MENTIONS, UNKNOWN_KIND, ingest_corpus,
    ingest_decisions, ingest_text,
};
pub use golden::{Expectation, GoldenQuestion, Graded, Scorecard, Verdict, parse_golden};
pub use parse::{ParseError, ParsedRecord, parse_corpus};
pub use register::{ParsedUnknown, Resolution, parse_register};

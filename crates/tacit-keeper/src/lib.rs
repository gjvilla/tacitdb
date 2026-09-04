//! The keeper layer: content, files, and corpus judgment.
//!
//! Everything that knows about markdown, the filesystem, or what a decision
//! record *means* lives here rather than in `tacit-core`, which owns only the
//! grammar (D-0002's two-layer bet, made a crate boundary).

pub mod attest;
pub mod corpus;
#[cfg(feature = "real-embedder")]
pub mod embed;
pub mod golden;
pub mod lock;
pub mod origin;
pub mod parse;
pub mod pep;
pub mod register;
pub mod synthetic;
pub mod verdict;

pub use corpus::{
    Attest, Attestations, DECISION_KIND, DECISIONS_DOC, Disposition, IngestError,
    IngestReport, MENTIONS, REGISTER_DOC, UNKNOWN_KIND, ingest_corpus, ingest_corpus_with,
    ingest_decisions, ingest_text, ingest_text_with,
};
pub use attest::{Attestation, Blame, Recheck, TrustReview, Verified, review_trust};
pub use origin::Origin;
pub use lock::{LockError, StoreLock, lock_path};
pub use verdict::{Ruling, render, retire_reason};
pub use golden::{
    Expectation, GoldenQuestion, Graded, Scorecard, Verdict, absent_vocabulary, missing_baseline, parse_baseline, parse_golden, quoted_questions,
    stale_triggers, vocabulary_drift, run_configured,
};
pub use parse::{ParseError, ParsedRecord, parse_corpus};
pub use pep::{PROPOSAL_KIND, Pep, PepError, PepReport, REQUIRES, Status, ingest_peps, parse_pep};
pub use register::{ParsedUnknown, Resolution, parse_register};
pub use synthetic::{Corpus, Shape, Topic, generate};

use crate::content::{Content, RecordKind};
use crate::envelope::{Author, Envelope, Evidence, ReviewTrigger, SourceRef};
use crate::id::RecordId;
use jiff::Timestamp;
use serde::Serialize;

/// A sealed, stored record. No public constructor, no `Deserialize`, no
/// mutators, and no state field: identity and record-time are engine-assigned
/// (invariant 3), the store is append-only (invariant 2), and state is derived
/// from verdicts (invariant 4).
#[derive(Debug, Clone, Serialize)]
pub struct Record {
    id: RecordId,
    envelope: Envelope,
    content: Content,
}

impl Record {
    pub(crate) fn new(id: RecordId, envelope: Envelope, content: Content) -> Self {
        Self { id, envelope, content }
    }

    pub fn id(&self) -> RecordId {
        self.id
    }

    pub fn envelope(&self) -> &Envelope {
        &self.envelope
    }

    pub fn content(&self) -> &Content {
        &self.content
    }

    pub fn kind(&self) -> RecordKind {
        self.content.kind()
    }
}

/// What callers construct. A draft carries everything the author controls and
/// nothing the engine assigns — the type is the boundary of invariant 1: there
/// is no way to hand the ledger content without an author and a source.
#[derive(Debug, Clone)]
pub struct Draft {
    pub author: Author,
    pub source: SourceRef,
    pub valid_from: Option<Timestamp>,
    pub valid_to: Option<Timestamp>,
    pub evidence: Vec<Evidence>,
    pub review_trigger: Option<ReviewTrigger>,
    pub supersedes: Option<RecordId>,
    pub content: Content,
}

impl Draft {
    pub fn new(author: Author, source: SourceRef, content: Content) -> Self {
        Self {
            author,
            source,
            valid_from: None,
            valid_to: None,
            evidence: Vec::new(),
            review_trigger: None,
            supersedes: None,
            content,
        }
    }
}

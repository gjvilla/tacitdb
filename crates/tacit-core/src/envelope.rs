use crate::id::{EntityId, RecordId};
use crate::validity::Validity;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

pub const ENVELOPE_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthorKind {
    Human,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Author {
    pub name: String,
    pub kind: AuthorKind,
    pub detail: Option<String>,
}

impl Author {
    pub fn human(name: impl Into<String>) -> Self {
        Self { name: name.into(), kind: AuthorKind::Human, detail: None }
    }

    pub fn agent(name: impl Into<String>) -> Self {
        Self { name: name.into(), kind: AuthorKind::Agent, detail: None }
    }
}

/// Where a record entered the record: interview, huddle, ingest, migration,
/// agent-pipeline. Open vocabulary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceRef {
    pub channel: String,
    pub reference: Option<String>,
}

impl SourceRef {
    pub fn channel(channel: impl Into<String>) -> Self {
        Self { channel: channel.into(), reference: None }
    }
}

/// Evidence must point at an entity of kind `"source"` (design/001 §1.4);
/// the ledger enforces that at append time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    pub source: EntityId,
    pub span: Option<String>,
}

/// The receipt a redaction rewrite leaves on what it withheld (U-11, D-0047).
///
/// `by` names the redaction record that ordered the removal — the permanent,
/// appended declaration of who, why, and how much — and a store refuses to
/// open if the mark points at nothing, so a hand-written husk cannot pose as
/// a lawful one. `fingerprint` is a 64-bit hash of the event line as it stood
/// before the rewrite: enough to *match* a retained original (a backup, an
/// upstream document) against what was removed, and deliberately not claimed
/// as cryptographic proof — that upgrade is named in the register rather than
/// implied here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RedactionMark {
    pub by: RecordId,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewTrigger {
    pub due_at: Option<Timestamp>,
    pub on_event: Option<String>,
}

/// The sealed envelope. Constructible only by the ledger (`recorded_at` is
/// engine-assigned — invariant 3), hence no `Deserialize` and no public
/// constructor: external code cannot forge a stored envelope.
#[derive(Debug, Clone, Serialize)]
pub struct Envelope {
    author: Author,
    source: SourceRef,
    recorded_at: Timestamp,
    valid_from: Timestamp,
    valid_to: Option<Timestamp>,
    evidence: Vec<Evidence>,
    review_trigger: Option<ReviewTrigger>,
    supersedes: Option<RecordId>,
    redacted: Option<RedactionMark>,
    version: u16,
}

impl Envelope {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn seal(
        author: Author,
        source: SourceRef,
        recorded_at: Timestamp,
        valid_from: Option<Timestamp>,
        valid_to: Option<Timestamp>,
        evidence: Vec<Evidence>,
        review_trigger: Option<ReviewTrigger>,
        supersedes: Option<RecordId>,
        redacted: Option<RedactionMark>,
    ) -> Self {
        Self {
            author,
            source,
            recorded_at,
            valid_from: valid_from.unwrap_or(recorded_at),
            valid_to,
            evidence,
            review_trigger,
            supersedes,
            redacted,
            version: ENVELOPE_VERSION,
        }
    }

    pub fn author(&self) -> &Author {
        &self.author
    }

    /// The receipt, when part of this record was withheld by a rewrite.
    /// `None` is the ordinary case: nothing was ever removed.
    pub fn redacted(&self) -> Option<&RedactionMark> {
        self.redacted.as_ref()
    }

    pub fn source(&self) -> &SourceRef {
        &self.source
    }

    pub fn recorded_at(&self) -> Timestamp {
        self.recorded_at
    }

    pub fn valid_from(&self) -> Timestamp {
        self.valid_from
    }

    pub fn valid_to(&self) -> Option<Timestamp> {
        self.valid_to
    }

    /// The valid-time interval, half-open `[from, to)`. The ledger rejects
    /// empty intervals at append time, so this never returns `None`.
    pub fn validity(&self) -> Validity {
        Validity::new(self.valid_from, self.valid_to).expect("append rejects empty validity")
    }

    pub fn evidence(&self) -> &[Evidence] {
        &self.evidence
    }

    pub fn review_trigger(&self) -> Option<&ReviewTrigger> {
        self.review_trigger.as_ref()
    }

    pub fn supersedes(&self) -> Option<RecordId> {
        self.supersedes
    }

    pub fn version(&self) -> u16 {
        self.version
    }
}

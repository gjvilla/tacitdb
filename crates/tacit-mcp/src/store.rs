//! The store the host serves: a ledger with its derived indexes, plus the
//! audit log every tool call writes to (R-11).

use jiff::Timestamp;
use serde::Serialize;
use tacit_core::{HashingEmbedder, Ledger, Projection, TextIndex, VectorIndex};

/// Bounded so a long-running host cannot grow without limit; the oldest
/// entries fall off rather than being silently discarded on write.
const AUDIT_CAPACITY: usize = 1_000;

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct AuditEntry {
    pub at: String,
    pub tool: String,
    pub detail: String,
    pub outcome: String,
}

pub struct Store {
    pub ledger: Ledger,
    pub projection: Projection,
    pub index: TextIndex,
    pub vectors: VectorIndex,
    pub embedder: HashingEmbedder,
    audit: Vec<AuditEntry>,
    dropped: usize,
}

impl Store {
    pub fn new(ledger: Ledger) -> Self {
        let projection = Projection::rebuild(&ledger);
        let index = TextIndex::rebuild(&ledger);
        let embedder = HashingEmbedder::default();
        let vectors = VectorIndex::rebuild(&ledger, &embedder);
        Self { ledger, projection, index, vectors, embedder, audit: Vec::new(), dropped: 0 }
    }

    /// Bring the derived indexes level with the log. Called after every write,
    /// because a tool that appended and did not advance would answer the next
    /// read from a stale view.
    pub fn refresh(&mut self) {
        self.projection.advance(&self.ledger);
        self.index.advance(&self.ledger);
        self.vectors.advance(&self.ledger, &self.embedder);
    }

    pub fn record_call(&mut self, tool: &str, detail: impl Into<String>, outcome: impl Into<String>) {
        if self.audit.len() >= AUDIT_CAPACITY {
            self.audit.remove(0);
            self.dropped += 1;
        }
        self.audit.push(AuditEntry {
            at: Timestamp::now().strftime("%Y-%m-%dT%H:%M:%SZ").to_string(),
            tool: tool.to_string(),
            detail: detail.into(),
            outcome: outcome.into(),
        });
    }

    pub fn audit(&self, limit: usize) -> (Vec<AuditEntry>, usize) {
        let start = self.audit.len().saturating_sub(limit);
        (self.audit[start..].to_vec(), self.dropped)
    }
}

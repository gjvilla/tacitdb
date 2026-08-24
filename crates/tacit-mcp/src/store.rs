//! The store the host serves: a ledger with its derived indexes, plus the
//! audit log every tool call writes to (R-11).

use jiff::Timestamp;
use serde::Serialize;
use tacit_core::{MemoryLedger, Projection, TextIndex};

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
    pub ledger: MemoryLedger,
    pub projection: Projection,
    pub index: TextIndex,
    audit: Vec<AuditEntry>,
    dropped: usize,
}

impl Store {
    pub fn new(ledger: MemoryLedger) -> Self {
        let projection = Projection::rebuild(&ledger);
        let index = TextIndex::rebuild(&ledger);
        Self { ledger, projection, index, audit: Vec::new(), dropped: 0 }
    }

    /// Bring the derived indexes level with the log. Called after every write,
    /// because a tool that appended and did not advance would answer the next
    /// read from a stale view.
    pub fn refresh(&mut self) {
        self.projection.advance(&self.ledger);
        self.index.advance(&self.ledger);
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

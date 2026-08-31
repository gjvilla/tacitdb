//! The store the host serves: a ledger with its derived indexes, plus the
//! audit log every tool call writes to (R-11).
//!
//! With a durable store, the audit persists beside it — because U-3's
//! trigger reads "observed real agent usage of the v1 MCP toolset", and an
//! audit that dies with the process makes that trigger unfireable: usage
//! happened, nobody could ever observe it. The file is telemetry, not the
//! record: plain JSON lines, appended and flushed but not fsynced, read
//! back at open so the audit tool spans restarts. Losing a tail line in a
//! crash loses a data point about usage, never knowledge.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use tacit_core::{HashingEmbedder, Ledger, Projection, TextIndex, VectorIndex};

/// Bounded so a long-running host cannot grow without limit; the oldest
/// entries fall off rather than being silently discarded on write.
const AUDIT_CAPACITY: usize = 1_000;

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
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
    audit_path: Option<PathBuf>,
}

impl Store {
    pub fn new(ledger: Ledger) -> Self {
        let projection = Projection::rebuild(&ledger);
        let index = TextIndex::rebuild(&ledger);
        let embedder = HashingEmbedder::default();
        let vectors = VectorIndex::rebuild(&ledger, &embedder);
        Self {
            ledger,
            projection,
            index,
            vectors,
            embedder,
            audit: Vec::new(),
            dropped: 0,
            audit_path: None,
        }
    }

    /// Persist the audit at `path`, loading whatever an earlier run left
    /// there so the audit tool answers across restarts. A line that does not
    /// parse is counted as dropped rather than refusing the whole file —
    /// telemetry does not get to hold the store hostage.
    pub fn with_audit(mut self, path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        if let Ok(text) = std::fs::read_to_string(&path) {
            for line in text.lines().filter(|l| !l.trim().is_empty()) {
                match serde_json::from_str::<AuditEntry>(line) {
                    Ok(entry) => {
                        if self.audit.len() >= AUDIT_CAPACITY {
                            self.audit.remove(0);
                            self.dropped += 1;
                        }
                        self.audit.push(entry);
                    }
                    Err(_) => self.dropped += 1,
                }
            }
        }
        self.audit_path = Some(path);
        self
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
        let entry = AuditEntry {
            at: Timestamp::now().strftime("%Y-%m-%dT%H:%M:%SZ").to_string(),
            tool: tool.to_string(),
            detail: detail.into(),
            outcome: outcome.into(),
        };
        // Best-effort by design: the audit observes the store and must never
        // be the reason a read fails. The file only ever grows; the in-memory
        // ring is the bounded view of its tail.
        if let Some(path) = &self.audit_path
            && let Ok(line) = serde_json::to_string(&entry)
            && let Ok(mut file) =
                std::fs::OpenOptions::new().create(true).append(true).open(path)
        {
            let _ = writeln!(file, "{line}");
        }
        self.audit.push(entry);
    }

    pub fn audit(&self, limit: usize) -> (Vec<AuditEntry>, usize) {
        let start = self.audit.len().saturating_sub(limit);
        (self.audit[start..].to_vec(), self.dropped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// U-3's trigger reads "observed real agent usage", and an audit that
    /// died with the process made it unfireable. This holds the fix down:
    /// what one host records, the next host can still answer for.
    #[test]
    fn the_audit_outlives_the_process_that_wrote_it() {
        let mut path = std::env::temp_dir();
        path.push(format!("tacit-audit-test-{}.audit", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let mut store = Store::new(Ledger::new()).with_audit(&path);
        store.record_call("tacit_search", "why is the sky described as blue", "matches");
        drop(store);

        let reopened = Store::new(Ledger::new()).with_audit(&path);
        let (entries, dropped) = reopened.audit(10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].tool, "tacit_search");
        assert_eq!(dropped, 0);
        let _ = std::fs::remove_file(&path);
    }
}

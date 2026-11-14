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
use std::time::SystemTime;
use tacit_core::{HashingEmbedder, Ledger, Projection, TextIndex, VectorIndex};
use tacit_keeper::StoreLock;

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
    /// The store's lock, when this host holds it. Kept here rather than in
    /// `main` so a host that started as a reader can take it later.
    lock: Option<StoreLock>,
    /// Present while this host is a reader of a store another process holds.
    following: Option<Following>,
}

/// A store this host reads and does not hold (found 2026-11-14: every Claude
/// session launches its own host over stdio, the second found the lock held
/// and exited, and its session had no record at all). One process still
/// writes a store at a time — that is D-0055 and it is untouched — but a
/// second host no longer has to be blind to be harmless. It replays the log
/// without touching it, replays it again when the file has changed, and
/// refuses writes by name until the holder is gone, at which point the next
/// write takes the lock and this host becomes the holder.
struct Following {
    path: PathBuf,
    /// Who held the store when this host last asked, in the lock's words.
    holder: String,
    /// The log's length and modification time at the last replay.
    seen: Option<(u64, SystemTime)>,
}

fn stamp(path: &Path) -> Option<(u64, SystemTime)> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.len(), meta.modified().ok()?))
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
            lock: None,
            following: None,
        }
    }

    /// Hold `lock` for as long as this store is served.
    pub fn holding(mut self, lock: StoreLock) -> Self {
        self.lock = Some(lock);
        self
    }

    /// Serve the log at `path` as a reader, because `holder` has the store.
    pub fn following(path: impl AsRef<Path>, holder: impl Into<String>) -> Result<Self, tacit_core::Error> {
        let path = path.as_ref().to_path_buf();
        // Stamped before the read: a write landing between the two is then
        // seen as a change next time, at the cost of one spare replay.
        let seen = stamp(&path);
        let ledger = Ledger::open_read_only(&path)?.ledger;
        let mut store = Self::new(ledger);
        store.following = Some(Following { path, holder: holder.into(), seen });
        Ok(store)
    }

    /// The holder's name while this host only reads; `None` once it writes.
    pub fn read_only_because(&self) -> Option<&str> {
        self.following.as_ref().map(|f| f.holder.as_str())
    }

    fn replace_ledger(&mut self, ledger: Ledger) {
        self.projection = Projection::rebuild(&ledger);
        self.index = TextIndex::rebuild(&ledger);
        self.vectors = VectorIndex::rebuild(&ledger, &self.embedder);
        self.ledger = ledger;
    }

    /// Before a read: if this host follows a store and the log has changed
    /// since it last looked, replay it. A replay that fails — the holder
    /// caught mid-rewrite — keeps the view it had, which was true a moment
    /// ago, and tries again on the next call.
    pub fn catch_up(&mut self) {
        let Some(following) = &self.following else { return };
        let now = stamp(&following.path);
        if now == following.seen {
            return;
        }
        if let Ok(opened) = Ledger::open_read_only(&following.path) {
            self.replace_ledger(opened.ledger);
            if let Some(following) = &mut self.following {
                following.seen = now;
            }
        }
    }

    /// Before a write: a holder may write; a reader asks for the lock. If
    /// the holder has gone, this host takes it, replays the log as a holder
    /// does, and writes from then on. If not, the refusal says who has the
    /// store and that reading still works — the write is the only thing a
    /// second host cannot do.
    pub fn writable(&mut self) -> Result<(), String> {
        let Some(following) = &self.following else { return Ok(()) };
        let path = following.path.clone();
        match StoreLock::acquire(&path, "tacit-mcp") {
            Ok(lock) => {
                let opened = Ledger::open(&path)
                    .map_err(|e| format!("could not open {} to write: {e}", path.display()))?;
                self.replace_ledger(opened.ledger);
                self.lock = Some(lock);
                self.following = None;
                Ok(())
            }
            Err(error) => {
                let message = error.to_string();
                if let Some(following) = &mut self.following
                    && let tacit_keeper::LockError::Held { holder, pid, .. } = &error
                {
                    following.holder = format!("{holder} (pid {pid})");
                }
                Err(format!(
                    "this host is read-only, so nothing was written: {message}. Searching and \
                     reading still work here and see what the holder writes. To record this, \
                     use the session whose host holds the store, or retry once it has exited — \
                     this host takes over on the first write after that."
                ))
            }
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

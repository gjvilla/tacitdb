//! Durability as an append-only event log, replayed through the grammar.
//!
//! The load path is the whole design problem. [`Record`] and [`Envelope`]
//! deliberately have no `Deserialize`, so the obvious move — read records off
//! disk and hand them to the ledger — would make every invariant true only of
//! records that happened to arrive through `append`. Since a durable store is
//! where records spend most of their life, that would leave the ratchet
//! guarding an empty doorway.
//!
//! So nothing is deserialized into a sealed type. The log stores *events*: the
//! author's draft plus the two facts the engine assigned it (its id and its
//! record-time). Loading replays each event through the same validation an
//! append runs — evidence must still point at a source, entity refs must still
//! resolve, and a verdict must still be legal against the state built from the
//! events before it.
//!
//! The consequence worth stating plainly: **the store is not trusted, it is
//! re-validated.** A hand-edited file cannot smuggle in a promoted claim,
//! because promotion is not a field anyone can write — it is a fold over
//! verdicts, and a forged verdict has to survive the same grammar a live one
//! does.

use crate::content::Content;
use crate::envelope::{Author, Evidence, RedactionMark, ReviewTrigger, SourceRef};
use crate::error::Error;
use crate::id::{EntityId, RecordId};
use crate::measurement::MeasurementTarget;
use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// One line of the log. Deliberately a *wire* type, distinct from the sealed
/// in-memory types: what is written down is what the author supplied plus what
/// the engine assigned, never a record that could be read back whole.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum Event {
    Entity {
        id: EntityId,
        kind: String,
        label: String,
        /// The receipt an entity-label redaction left (U-46, D-0053). Only
        /// the rewrite writes it, and a mark naming no declaration refuses
        /// to load, exactly as a record husk's does.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        redacted: Option<RedactionMark>,
    },
    Record {
        id: RecordId,
        recorded_at: Timestamp,
        envelope_version: u16,
        author: Author,
        source: SourceRef,
        valid_from: Timestamp,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        valid_to: Option<Timestamp>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        evidence: Vec<Evidence>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        review_trigger: Option<ReviewTrigger>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        supersedes: Option<RecordId>,
        /// The receipt a redaction rewrite left, when part of this event was
        /// withheld (U-11). Only a rewrite writes it; a live append cannot,
        /// and a mark that names no redaction record refuses to load.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        redacted: Option<RedactionMark>,
        content: Content,
    },
    /// Measurements are mutable in place, so the log is last-write-wins on
    /// replay — the instrument panel has no history to preserve (D-0013).
    Measurement {
        target: MeasurementTarget,
        name: String,
        value: f64,
        at: Timestamp,
        by: Author,
    },
}

/// The open log a ledger appends to.
#[derive(Debug)]
pub struct Journal {
    path: PathBuf,
    file: File,
}

impl Journal {
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn open_for_append(path: &Path) -> Result<Self, Error> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| Error::Storage { path: path.into(), detail: e.to_string() })?;
        Ok(Self { path: path.to_path_buf(), file })
    }

    /// Write one event and put it on the disk before returning. The caller
    /// commits to memory only after this succeeds, so a failed write leaves
    /// the ledger exactly as it was rather than ahead of its own log.
    pub(crate) fn append(&mut self, event: &Event) -> Result<(), Error> {
        let fail = |detail: String| Error::Storage { path: self.path.clone(), detail };
        let mut line =
            serde_json::to_vec(event).map_err(|e| fail(format!("encoding event: {e}")))?;
        line.push(b'\n');
        self.file.write_all(&line).map_err(|e| fail(e.to_string()))?;
        self.file.sync_data().map_err(|e| fail(e.to_string()))?;
        Ok(())
    }
}

/// What `open` found on disk.
#[derive(Debug)]
pub struct Recovery {
    pub events_replayed: usize,
    /// Bytes dropped from a torn final line. A crash between `write` and
    /// `sync_data` leaves a partial record that no reader ever saw, so
    /// discarding it restores the log to its last consistent state. Anything
    /// malformed *before* the end is corruption and refuses to load.
    pub truncated_bytes: u64,
    /// How far the log's last record-time leads the wall clock, if it does.
    /// Set by `Ledger::open` after replay. Present means the machine's clock
    /// stepped backwards since the log was written: reads are unaffected, and
    /// appends hold or refuse per U-22 until the clock catches up.
    pub leads_clock: Option<SignedDuration>,
}

/// Read the log, discarding a torn final line, and hand back the events for
/// replay together with the journal to continue appending to.
pub(crate) fn read(path: &Path) -> Result<(Vec<Event>, Journal, Recovery), Error> {
    let corrupt = |line: usize, detail: String| Error::CorruptJournal {
        path: path.to_path_buf(),
        line,
        detail,
    };

    let mut events = Vec::new();
    let mut truncated_bytes = 0u64;

    if path.exists() {
        let text = std::fs::read_to_string(path)
            .map_err(|e| Error::Storage { path: path.into(), detail: e.to_string() })?;

        // Byte offsets are tracked rather than inferred: a torn tail has no
        // trailing newline, so computing the drop from the line length alone
        // eats the previous line's terminator.
        let chunks: Vec<&str> = text.split_inclusive('\n').collect();
        let last = chunks.len();
        let mut kept_bytes = 0u64;

        for (index, chunk) in chunks.iter().enumerate() {
            let line = chunk.strip_suffix('\n').unwrap_or(chunk);
            if line.trim().is_empty() {
                kept_bytes += chunk.len() as u64;
                continue;
            }
            match serde_json::from_str::<Event>(line) {
                Ok(event) => {
                    events.push(event);
                    kept_bytes += chunk.len() as u64;
                }
                // A torn tail: the append never completed, so no reader ever
                // saw this record. Dropping it restores the last consistent
                // state. Damage anywhere earlier is corruption.
                Err(_) if index + 1 == last => {
                    truncated_bytes = chunk.len() as u64;
                }
                Err(error) => return Err(corrupt(index + 1, error.to_string())),
            }
        }

        if truncated_bytes > 0 {
            let file = OpenOptions::new()
                .write(true)
                .open(path)
                .map_err(|e| Error::Storage { path: path.into(), detail: e.to_string() })?;
            file.set_len(kept_bytes)
                .map_err(|e| Error::Storage { path: path.into(), detail: e.to_string() })?;
        }
    }

    let journal = Journal::open_for_append(path)?;
    let recovery =
        Recovery { events_replayed: events.len(), truncated_bytes, leads_clock: None };
    Ok((events, journal, recovery))
}

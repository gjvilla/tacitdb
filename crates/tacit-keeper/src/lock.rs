//! One process holds a store at a time.
//!
//! D-0015 stated it as a property of the runtime shape — "one process owns a
//! store at a time (file-lock semantics)" — and for a year of days it was true
//! because only the MCP host ever opened a store. The verdict command is a
//! second process that writes the same log (D-0055), which is the event
//! D-0022 named as its review trigger, and this module is the re-read's
//! answer: two processes never write one log, because the second cannot
//! open it.
//!
//! Why it matters more than politeness: the host holds the ledger in memory
//! and appends to the file's end. A verdict appended underneath it by another
//! process would carry a later record-time than the host's next append, and
//! the log would stop replaying — refused by the same monotonicity check that
//! protects `state_of_at`. The lock keeps that from being possible rather than
//! merely unlikely.
//!
//! Mechanism, dependency-free by design (R-4): a sidecar file beside the
//! store, created exclusively, holding the holder's pid and name, removed
//! when the holder drops it. A holder that died without dropping leaves the
//! file behind, so an existing lock is believed only while its pid is alive;
//! a dead holder's lock is taken over and said so. Stated limit: liveness is
//! asked with `kill -0`, which also fails for a live process owned by another
//! user, so on a shared machine a holder running as someone else reads as
//! gone. Single-tenant is the v1 regime (D-0015); the limit is named here so
//! it is not discovered.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum LockError {
    #[error(
        "{store} is held by {holder} (pid {pid}) — stop it first, or wait for it; the lock is \
         {lock}"
    )]
    Held { store: PathBuf, lock: PathBuf, holder: String, pid: u32 },

    #[error("could not take the lock {lock}: {source}")]
    Io {
        lock: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Held for as long as the value lives; released on drop.
#[derive(Debug)]
pub struct StoreLock {
    path: PathBuf,
    /// Present when a dead holder's lock was taken over, so the caller can
    /// say so rather than have it happen silently.
    pub took_over_from: Option<String>,
}

/// Where a store's lock lives: `acme.log` is held by `acme.lock`, beside the
/// audit at `acme.audit`.
pub fn lock_path(store: &Path) -> PathBuf {
    store.with_extension("lock")
}

impl StoreLock {
    /// Take the lock for `store`, naming the holder (`tacit-mcp`,
    /// `tacit-keeper verdict`) so the refusal can say who has it.
    pub fn acquire(store: &Path, holder: &str) -> Result<Self, LockError> {
        let lock = lock_path(store);
        let io = |source: std::io::Error| LockError::Io { lock: lock.clone(), source };
        let mut took_over_from = None;
        // Two attempts: one against whatever is there, one after a dead
        // holder's file is cleared. A third failure is an error, not a loop.
        for _ in 0..2 {
            match OpenOptions::new().write(true).create_new(true).open(&lock) {
                Ok(mut file) => {
                    writeln!(file, "{} {holder}", std::process::id()).map_err(io)?;
                    file.sync_all().map_err(io)?;
                    return Ok(Self { path: lock, took_over_from });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    let contents = fs::read_to_string(&lock).unwrap_or_default();
                    let (pid, name) = parse_holder(&contents);
                    if let Some(pid) = pid
                        && alive(pid)
                    {
                        return Err(LockError::Held {
                            store: store.to_path_buf(),
                            lock,
                            holder: name,
                            pid,
                        });
                    }
                    took_over_from = Some(match pid {
                        Some(pid) => format!("{name} (pid {pid}, no longer running)"),
                        None => "an unreadable lock file".to_string(),
                    });
                    match fs::remove_file(&lock) {
                        Ok(()) => {}
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(error) => return Err(io(error)),
                    }
                }
                Err(error) => return Err(io(error)),
            }
        }
        Err(LockError::Io {
            lock: lock.clone(),
            source: std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "the lock reappeared as fast as it was cleared",
            ),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for StoreLock {
    fn drop(&mut self) {
        // Best effort: a lock that cannot be removed is exactly the case the
        // liveness check exists for, and the next holder will take it over.
        let _ = fs::remove_file(&self.path);
    }
}

fn parse_holder(contents: &str) -> (Option<u32>, String) {
    let line = contents.lines().next().unwrap_or("");
    let (pid, name) = line.split_once(' ').unwrap_or((line, "an unnamed process"));
    (pid.trim().parse().ok(), name.trim().to_string())
}

fn alive(pid: u32) -> bool {
    if pid == std::process::id() {
        return true;
    }
    #[cfg(unix)]
    {
        // A `kill` that cannot be run at all says nothing about the pid; the
        // safe reading of nothing is "still held".
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(true)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tacit-lock-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.join("store.log")
    }

    #[test]
    fn a_held_store_refuses_a_second_holder_and_names_the_first() {
        let store = scratch("held");
        let first = StoreLock::acquire(&store, "tacit-mcp").unwrap();
        let err = StoreLock::acquire(&store, "tacit-keeper verdict").unwrap_err();
        match err {
            LockError::Held { holder, pid, .. } => {
                assert_eq!(holder, "tacit-mcp");
                assert_eq!(pid, std::process::id());
            }
            other => panic!("expected Held, got {other}"),
        }
        drop(first);
        StoreLock::acquire(&store, "tacit-keeper verdict").expect("released on drop");
    }

    #[test]
    fn dropping_removes_the_file() {
        let store = scratch("drop");
        let lock = StoreLock::acquire(&store, "x").unwrap();
        assert!(lock.path().exists());
        let path = lock.path().to_path_buf();
        drop(lock);
        assert!(!path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_dead_holders_lock_is_taken_over_and_said_so() {
        let store = scratch("stale");
        // A process that has certainly exited, so its pid is certainly not alive.
        let child = std::process::Command::new("true").spawn().unwrap();
        let pid = child.id();
        let mut child = child;
        child.wait().unwrap();
        fs::write(lock_path(&store), format!("{pid} tacit-mcp\n")).unwrap();
        let lock = StoreLock::acquire(&store, "tacit-keeper verdict").unwrap();
        let note = lock.took_over_from.as_deref().expect("takeover is reported");
        assert!(note.contains("tacit-mcp") && note.contains("no longer running"), "{note}");
    }

    #[test]
    fn an_unreadable_lock_file_is_taken_over_not_trusted() {
        let store = scratch("garbage");
        fs::write(lock_path(&store), "not a pid at all").unwrap();
        let lock = StoreLock::acquire(&store, "x").unwrap();
        assert!(lock.took_over_from.as_deref().unwrap().contains("unreadable"));
    }
}

//! Who wrote the words the ingest is about to transcribe as a person's verdict.
//!
//! The engine's rule (invariant 5) is that a verdict *declares* a human author
//! and the keeper authenticates it. The keeper did not. It read `state:
//! promoted` out of a markdown file and asserted, on the file's say-so, that
//! the named person had declared a promotion — so write access to
//! `docs/DECISIONS.md` was promotion authority, and there is no promote tool
//! anywhere on the MCP surface precisely because that authority is supposed to
//! be hard to reach (U-29).
//!
//! What can actually be established about a file is who committed the bytes and
//! whether that commit was signed. Note what that is *not*: it is not who made
//! the decision. A person may record a decision someone else made, so the
//! document's `author:` is the decider and the commit's author is the editor,
//! and demanding they match would be wrong. The editor is the one worth
//! attesting anyway, because the editor is who the threat is about.
//!
//! The answer is carried in `Author.detail` — the envelope field that exists to
//! say how an author is known — so it becomes part of the permanent record
//! rather than a check that happened once and left no trace. A reader can then
//! ask which promotions rest on nothing, which is a better answer than a
//! boolean gate that leaves no memory of having been open.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::process::Command;
use tacit_core::{ClaimState, Content, Ledger, RecordId, RecordState, VerdictAction};

/// What could be established about the edit that put a record's current text in
/// its document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attestation {
    /// A commit carries the text and git verified a signature over it.
    Signed { commit: String, by: String },
    /// A commit carries the text and no signature could be verified.
    Unsigned { commit: String, by: String },
    /// Nothing can be established.
    None { because: String },
}

impl Attestation {
    fn unattested(because: impl Into<String>) -> Self {
        Attestation::None { because: because.into() }
    }

    /// How much weight the attestation carries: signed beats unsigned beats
    /// nothing.
    fn weight(&self) -> u8 {
        match self {
            Attestation::Signed { .. } => 2,
            Attestation::Unsigned { .. } => 1,
            Attestation::None { .. } => 0,
        }
    }

    pub fn is_signed(&self) -> bool {
        matches!(self, Attestation::Signed { .. })
    }

    /// The weakest attestation over a span, which is the one that governs.
    ///
    /// A record's current text can come from several commits, and the question
    /// is whether anything untrusted could have been slipped into it — so one
    /// unsigned line makes the whole record unsigned, however well attested the
    /// lines around it are. Those words did get there unsigned.
    pub fn weakest<'a>(over: impl IntoIterator<Item = &'a Attestation>) -> Attestation {
        over.into_iter()
            .min_by_key(|a| a.weight())
            .cloned()
            .unwrap_or_else(|| Attestation::unattested("the span is empty"))
    }

    /// Read an attestation back out of an `Author.detail`. Returns `None` for
    /// anything this module did not write.
    pub fn parse(detail: &str) -> Option<Self> {
        let rest = detail.strip_prefix("attest:")?;
        let (kind, rest) = rest.split_once(' ')?;
        match kind {
            "signed" | "unsigned" => {
                let commit = rest.strip_prefix("commit:")?;
                let (commit, by) = commit.split_once(" by:")?;
                let (commit, by) = (commit.to_string(), by.to_string());
                if commit.is_empty() || by.is_empty() {
                    return None;
                }
                Some(if kind == "signed" {
                    Attestation::Signed { commit, by }
                } else {
                    Attestation::Unsigned { commit, by }
                })
            }
            "none" => {
                let because = rest.strip_prefix("because:")?;
                (!because.is_empty()).then(|| Attestation::unattested(because))
            }
            _ => None,
        }
    }
}

impl fmt::Display for Attestation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // The committer's name and not their address: the name is the
            // identity a reader recognises, and an email in a personal
            // project's records buys nothing to make up for carrying it.
            Attestation::Signed { commit, by } => {
                write!(f, "attest:signed commit:{commit} by:{by}")
            }
            Attestation::Unsigned { commit, by } => {
                write!(f, "attest:unsigned commit:{commit} by:{by}")
            }
            Attestation::None { because } => write!(f, "attest:none because:{because}"),
        }
    }
}

/// A document's attestation, line by line.
#[derive(Debug, Clone)]
pub struct Blame {
    by_line: Vec<Attestation>,
    /// Present when git could not speak about this file at all.
    whole_file: Option<Attestation>,
}

impl Blame {
    /// A document nothing can be said about — an unreadable repository, or a
    /// corpus supplied as text rather than as a tracked file.
    pub fn unattested(because: impl Into<String>) -> Self {
        Self { by_line: Vec::new(), whole_file: Some(Attestation::unattested(because)) }
    }

    /// The attestation governing a 1-based inclusive line span.
    pub fn over(&self, span: (usize, usize)) -> Attestation {
        if let Some(whole) = &self.whole_file {
            return whole.clone();
        }
        let (start, end) = span;
        let lo = start.saturating_sub(1);
        let hi = end.min(self.by_line.len());
        if lo >= hi {
            return Attestation::unattested("the document has no such lines");
        }
        Attestation::weakest(&self.by_line[lo..hi])
    }
}

/// Ask git who wrote each line of `relative` beneath `repo_root`.
///
/// Never fails: anything git cannot answer becomes an unattested document with
/// the reason attached, because "we could not check" is a finding and not an
/// error to abort an ingest over.
pub fn blame(repo_root: &Path, relative: &str) -> Blame {
    let porcelain = match run(repo_root, &["blame", "--line-porcelain", "--", relative]) {
        Ok(text) => text,
        Err(why) => return Blame::unattested(why),
    };

    let mut authors: BTreeMap<String, String> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut current = String::new();
    for line in porcelain.lines() {
        if let Some(name) = line.strip_prefix("author ") {
            authors.insert(current.clone(), name.to_string());
        } else if line.starts_with('\t') {
            order.push(std::mem::take(&mut current));
        } else if let Some((sha, _)) = line.split_once(' ')
            && sha.len() == 40
            && sha.bytes().all(|b| b.is_ascii_hexdigit())
        {
            current = sha.to_string();
        }
    }

    // One call for every commit involved rather than one per line: %G? is git's
    // own verdict on the signature, so the trust decision stays git's.
    let shas: Vec<&str> = {
        let mut seen: Vec<&str> = order.iter().map(String::as_str).collect();
        seen.sort_unstable();
        seen.dedup();
        seen.into_iter().filter(|s| !s.is_empty() && !is_uncommitted(s)).collect()
    };
    let mut signed: BTreeMap<String, bool> = BTreeMap::new();
    if !shas.is_empty() {
        let mut args = vec!["log", "--no-walk", "--format=%H %G?"];
        args.extend(shas.iter().copied());
        if let Ok(text) = run(repo_root, &args) {
            for line in text.lines() {
                if let Some((sha, status)) = line.split_once(' ') {
                    // G is a good signature; U is good from a key of unknown
                    // validity. Everything else — bad, expired, revoked, or
                    // absent — is not something to call signed.
                    signed.insert(sha.to_string(), matches!(status.trim(), "G" | "U"));
                }
            }
        }
    }

    let by_line = order
        .into_iter()
        .map(|sha| {
            if is_uncommitted(&sha) {
                return Attestation::unattested("these lines are not committed");
            }
            let by = authors.get(&sha).cloned().unwrap_or_else(|| "unknown".into());
            let commit = sha.chars().take(7).collect();
            if signed.get(&sha).copied().unwrap_or(false) {
                Attestation::Signed { commit, by }
            } else {
                Attestation::Unsigned { commit, by }
            }
        })
        .collect();

    Blame { by_line, whole_file: None }
}

fn is_uncommitted(sha: &str) -> bool {
    sha.bytes().all(|b| b == b'0')
}

fn run(repo_root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()
        .map_err(|e| format!("git could not be run here ({e})"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        let first = detail.lines().next().unwrap_or("git refused").trim().to_string();
        return Err(first);
    }
    String::from_utf8(output.stdout).map_err(|_| "git returned something unreadable".into())
}

/// Every promoted claim whose promotion cannot show anything for itself, with
/// what the verdict does say.
///
/// This is why the attestation is written into the record rather than merely
/// checked at the door. A policy decides what to do during one ingest; this
/// answers the question afterwards, across every verdict the ledger holds —
/// including ones transcribed long before any policy existed, and ones that
/// will always stay unattested because they were made while the document was
/// still an uncommitted draft.
///
/// A verdict carrying no attestation at all counts, because absence of evidence
/// is exactly what this list is for.
pub fn unattested_promotions(ledger: &Ledger) -> Vec<(RecordId, String)> {
    let mut found = Vec::new();
    for claim in ledger.promoted_claims() {
        for verdict in ledger.history(claim.id()) {
            let Content::Verdict(v) = verdict.content() else { continue };
            if !matches!(&v.action, VerdictAction::Promote { target, .. } if *target == claim.id()) {
                continue;
            }
            // The verdict that promoted it is the one that matters; a later
            // retirement says nothing about how it got in.
            if ledger.state_of(claim.id()) != Some(RecordState::Claim(ClaimState::Promoted)) {
                continue;
            }
            match verdict.envelope().author().detail.as_deref().map(Attestation::parse) {
                Some(Some(a)) if a.is_signed() => {}
                Some(Some(a)) => found.push((claim.id(), a.to_string())),
                Some(None) => found.push((claim.id(), "the author detail says nothing this keeper wrote".into())),
                None => found.push((claim.id(), "the verdict makes no attestation at all".into())),
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_attestation_round_trips_through_its_detail() {
        for attestation in [
            Attestation::Signed { commit: "a2b870e".into(), by: "Greg Villa".into() },
            Attestation::Unsigned { commit: "539b1c7".into(), by: "A Colleague".into() },
            Attestation::unattested("these lines are not committed"),
        ] {
            let rendered = attestation.to_string();
            assert_eq!(Attestation::parse(&rendered), Some(attestation), "from {rendered:?}");
        }
    }

    #[test]
    fn a_detail_this_module_did_not_write_parses_to_nothing() {
        for detail in ["", "a colleague vouched for it", "attest:signed", "attest:none because:"] {
            assert_eq!(Attestation::parse(detail), None, "from {detail:?}");
        }
    }

    #[test]
    fn one_unsigned_line_governs_the_span_it_sits_in() {
        let signed = Attestation::Signed { commit: "aaaaaaa".into(), by: "Greg".into() };
        let unsigned = Attestation::Unsigned { commit: "bbbbbbb".into(), by: "Greg".into() };
        let nothing = Attestation::unattested("these lines are not committed");

        // However well attested the lines around it: those words did get there
        // unsigned, and the span is only as trustworthy as its weakest line.
        assert_eq!(
            Attestation::weakest(&[signed.clone(), unsigned.clone(), signed.clone()]),
            unsigned
        );
        assert_eq!(Attestation::weakest(&[signed.clone(), unsigned, nothing.clone()]), nothing);
        assert_eq!(Attestation::weakest(&[signed.clone(), signed.clone()]), signed);
    }

    #[test]
    fn this_repository_attests_its_own_corpus() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let blame = blame(&root, "docs/DECISIONS.md");
        // Line 1 has been committed since the first commit, so whatever else is
        // true of the working tree, git can speak about it.
        assert!(
            !matches!(blame.over((1, 1)), Attestation::None { .. }),
            "git should be able to attest the first line of a tracked file"
        );
    }

    #[test]
    fn a_file_git_cannot_speak_about_is_unattested_rather_than_an_error() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let blame = blame(&root, "docs/NO-SUCH-FILE.md");
        assert!(matches!(blame.over((1, 10)), Attestation::None { .. }));
    }
}

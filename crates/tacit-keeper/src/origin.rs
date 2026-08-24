//! Where an ingested record came from, and what the document said when it did.
//!
//! The corpus documents are upstream: `docs/DECISIONS.md` is the copy a person
//! edits, and the ledger is downstream of it. That only works if a second
//! ingest can recognise what the first one wrote — otherwise a durable store
//! either duplicates its whole corpus on every run or freezes at whatever the
//! documents said the first time (U-19).
//!
//! Recognition needs two things in the record's own envelope: a stable name for
//! the thing the document is describing, and a fingerprint of what it said. Both
//! go in `SourceRef.reference`, which already exists to say where a record came
//! from — no new engine concept, and the provenance a reader sees is the same
//! string the sync parses.
//!
//! ```text
//! docs/DECISIONS.md#D-0001 recorded:2026-08-22 digest:3f2a91c4e0d18b76
//! docs/DECISIONS.md#D-0001/title recorded:2026-08-22 digest:3f2a91c4e0d18b76
//! docs/REGISTER.md#U-3 digest:9b4c0e17a2d5f836
//! ```
//!
//! Every record derived from one source record carries that source record's
//! digest, so "did D-0001 change?" is one comparison and its title and its
//! cross-references move together with it.

use std::fmt;

/// A record's place in the document it was ingested from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    /// Repo-relative path: `docs/DECISIONS.md`.
    pub document: String,
    /// What the document calls it, plus the role this record plays for it:
    /// `D-0001`, `D-0001/title`, `D-0001/mentions/D-0003`, `U-3`.
    pub key: String,
    /// The document's own stated record-time, carried verbatim rather than
    /// backdating the ledger's (invariant 3).
    pub noted: Option<String>,
    /// A fingerprint of the source text this record was derived from.
    pub digest: String,
}

impl Origin {
    pub fn new(document: &str, key: impl Into<String>, digest: &str) -> Self {
        Self {
            document: document.to_string(),
            key: key.into(),
            noted: None,
            digest: digest.to_string(),
        }
    }

    pub fn noted(mut self, noted: Option<&str>) -> Self {
        self.noted = noted.map(str::to_string);
        self
    }

    /// The same origin in a different role for the same source record — the
    /// digest travels, which is what makes a changed record supersede all of
    /// its derived claims together.
    pub fn role(&self, suffix: &str) -> Self {
        Self { key: format!("{}/{suffix}", self.key), ..self.clone() }
    }

    /// `document#key`, without the digest: the identity that persists across
    /// edits, and what the sync looks a prior ingest up by.
    pub fn identity(&self) -> String {
        format!("{}#{}", self.document, self.key)
    }

    /// Read an origin back out of a `SourceRef.reference`. Returns `None` for
    /// any reference this module did not write, which is the honest answer:
    /// a record whose provenance is not a corpus ingest is not one the sync
    /// may claim to own.
    pub fn parse(reference: &str) -> Option<Self> {
        let (head, digest) = reference.rsplit_once(" digest:")?;
        if digest.len() != 16 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let (document, rest) = head.split_once('#')?;
        let (key, noted) = match rest.split_once(" recorded:") {
            Some((key, noted)) => (key, Some(noted.to_string())),
            None => (rest, None),
        };
        if document.is_empty() || key.is_empty() || key.contains(char::is_whitespace) {
            return None;
        }
        Some(Self {
            document: document.to_string(),
            key: key.to_string(),
            noted,
            digest: digest.to_string(),
        })
    }
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{}", self.document, self.key)?;
        if let Some(noted) = &self.noted {
            write!(f, " recorded:{noted}")?;
        }
        write!(f, " digest:{}", self.digest)
    }
}

/// A change detector over the source text, not a tamper seal.
///
/// FNV-1a is trivially collidable on purpose-built input, and that costs
/// nothing here: whoever can craft a colliding edit to `docs/DECISIONS.md`
/// can also just write whatever they like in it. The threat this guards
/// against is a re-run of an unchanged document, not an adversary — and
/// pretending otherwise by reaching for a cryptographic hash would be
/// security theatre in an envelope that already trusts the file.
pub fn digest(text: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_origin_round_trips_through_its_reference() {
        for origin in [
            Origin::new("docs/DECISIONS.md", "D-0001", &digest("body")).noted(Some("2026-08-22")),
            Origin::new("docs/REGISTER.md", "U-3", &digest("row")),
            Origin::new("docs/DECISIONS.md", "D-0001/mentions/D-0003", &digest("body")),
        ] {
            let rendered = origin.to_string();
            assert_eq!(Origin::parse(&rendered), Some(origin), "from {rendered:?}");
        }
    }

    #[test]
    fn a_reference_the_sync_did_not_write_parses_to_nothing() {
        // The old format, a hand-written reference, and a truncated digest.
        // Claiming any of these would let the sync supersede a record it has
        // no business owning.
        for reference in [
            "docs/DECISIONS.md D-0001 recorded:2026-08-22",
            "a conversation on Tuesday",
            "docs/DECISIONS.md#D-0001 digest:beef",
            "docs/DECISIONS.md#D-0001 digest:zzzzzzzzzzzzzzzz",
            "#D-0001 digest:3f2a91c4e0d18b76",
        ] {
            assert_eq!(Origin::parse(reference), None, "from {reference:?}");
        }
    }

    #[test]
    fn the_digest_moves_with_the_text_and_the_role_does_not() {
        let a = Origin::new("docs/DECISIONS.md", "D-0001", &digest("one"));
        let b = Origin::new("docs/DECISIONS.md", "D-0001", &digest("two"));
        assert_ne!(a.digest, b.digest);
        assert_eq!(a.identity(), b.identity());
        assert_eq!(a.role("title").digest, a.digest);
        assert_eq!(a.role("title").identity(), "docs/DECISIONS.md#D-0001/title");
    }
}

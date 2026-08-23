use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// A valid-time interval, half-open: `[from, to)`. One definition, used by
/// both the ledger's contradiction check and the projection's view filter —
/// two definitions of "valid" in two files is how U-14's edge cases arrive.
///
/// Consequence of half-openness worth knowing: an instantaneous fact is not
/// representable (`from == to` is an empty, rejected interval). A caller with
/// a point observation must choose an explicit width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Validity {
    from: Timestamp,
    to: Option<Timestamp>,
}

impl Validity {
    pub fn new(from: Timestamp, to: Option<Timestamp>) -> Option<Self> {
        match to {
            Some(end) if end <= from => None,
            _ => Some(Self { from, to }),
        }
    }

    pub fn from_open(from: Timestamp) -> Self {
        Self { from, to: None }
    }

    pub fn from(&self) -> Timestamp {
        self.from
    }

    pub fn to(&self) -> Option<Timestamp> {
        self.to
    }

    /// `from <= t < to`.
    pub fn contains(&self, t: Timestamp) -> bool {
        self.from <= t && self.to.is_none_or(|end| t < end)
    }

    pub fn overlaps(&self, other: &Validity) -> bool {
        let self_ends_after_other_starts = self.to.is_none_or(|end| end > other.from);
        let other_ends_after_self_starts = other.to.is_none_or(|end| end > self.from);
        self_ends_after_other_starts && other_ends_after_self_starts
    }
}

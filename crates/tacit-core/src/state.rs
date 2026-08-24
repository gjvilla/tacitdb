use crate::content::ScoreOutcome;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClaimState {
    Proposed,
    Promoted,
    Retired,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GapState {
    Registered,
    Answered,
    Withdrawn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HypothesisState {
    Registered,
    Scored(ScoreOutcome),
    /// Stopped being predicted before its score date — the hypothesis
    /// equivalent of a withdrawn question, and not the same thing as one that
    /// was scored `Falsified`.
    Abandoned,
}

/// Derived, never stored: state is a fold over the verdicts touching a record
/// (invariant 4). Verdicts themselves are immutable and stateless.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordState {
    Claim(ClaimState),
    Gap(GapState),
    Hypothesis(HypothesisState),
    Verdict,
}

impl fmt::Display for RecordState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecordState::Claim(s) => write!(f, "claim:{s:?}"),
            RecordState::Gap(s) => write!(f, "gap:{s:?}"),
            RecordState::Hypothesis(s) => write!(f, "hypothesis:{s:?}"),
            RecordState::Verdict => f.write_str("verdict"),
        }
    }
}

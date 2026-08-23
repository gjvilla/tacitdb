use crate::id::{EntityId, RecordId};
use crate::state::{ClaimState, GapState, HypothesisState, RecordState};
use crate::value::Value;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordKind {
    Claim,
    Gap,
    Hypothesis,
    Verdict,
}

impl fmt::Display for RecordKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            RecordKind::Claim => "claim",
            RecordKind::Gap => "gap",
            RecordKind::Hypothesis => "hypothesis",
            RecordKind::Verdict => "verdict",
        };
        f.write_str(s)
    }
}

/// Record content. The kind is implied by the variant, so a kind/content
/// mismatch is unrepresentable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Content {
    Claim(ClaimContent),
    Gap(GapContent),
    Hypothesis(HypothesisContent),
    Verdict(VerdictContent),
}

impl Content {
    pub fn kind(&self) -> RecordKind {
        match self {
            Content::Claim(_) => RecordKind::Claim,
            Content::Gap(_) => RecordKind::Gap,
            Content::Hypothesis(_) => RecordKind::Hypothesis,
            Content::Verdict(_) => RecordKind::Verdict,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClaimContent {
    Attribute {
        subject: EntityId,
        name: String,
        value: Value,
    },
    Relation {
        subject: EntityId,
        predicate: String,
        object: EntityId,
        properties: BTreeMap<String, Value>,
    },
    /// The pattern-language unit: a solution bound to the forces that make it
    /// true (design/001 §1.2).
    Pattern {
        context: String,
        forces: Vec<String>,
        solution: String,
        /// Entity refs, mirroring `GapContent::territory`. Prose claims would
        /// otherwise be invisible to entity-scoped retrieval and to the
        /// projection, and append-only means the field cannot be added to
        /// records already written. Empty is legal and honest.
        about: Vec<EntityId>,
    },
    Text {
        body: String,
        about: Vec<EntityId>,
    },
}

impl ClaimContent {
    /// Every entity this claim references — the ledger checks each exists.
    pub fn entity_refs(&self) -> Vec<EntityId> {
        match self {
            ClaimContent::Attribute { subject, .. } => vec![*subject],
            ClaimContent::Relation { subject, object, .. } => vec![*subject, *object],
            ClaimContent::Pattern { about, .. } | ClaimContent::Text { about, .. } => about.clone(),
        }
    }

    /// The unordered set of entities this claim is *about*, which must not
    /// repeat — a repeat would multiply the record across entity-scoped reads.
    /// Distinct from `entity_refs`: a self-relation names one entity in two
    /// different roles, which is meaningful rather than duplicated.
    pub fn ref_list(&self) -> &[EntityId] {
        match self {
            ClaimContent::Pattern { about, .. } | ClaimContent::Text { about, .. } => about,
            _ => &[],
        }
    }
}

/// A registered known-unknown: a named question without an agreed answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GapContent {
    pub question: String,
    pub territory: Vec<EntityId>,
}

/// A dated, falsifiable prediction (the H-0001 shape).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HypothesisContent {
    pub statement: String,
    pub falsifier: Option<String>,
    pub score_by: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScoreOutcome {
    Met,
    Falsified,
    Inconclusive,
}

/// Why a claim left the promoted set. Retirement is not deletion, and the
/// three reasons carry different meanings for drift analysis: a superseded
/// claim was replaced, a no-longer-true claim marks real-world change, and a
/// promoted-in-error claim marks a verdict mistake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetireReason {
    Superseded,
    NoLongerTrue,
    PromotedInError,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VerdictAction {
    /// One verdict may promote a superseding claim and retire the record it
    /// supersedes — one decision, both transitions (design/001 §3.1).
    Promote {
        target: RecordId,
        retiring: Option<RecordId>,
    },
    Retire {
        target: RecordId,
        reason: RetireReason,
    },
    Reject {
        target: RecordId,
    },
    Answer {
        gap: RecordId,
        with_claim: RecordId,
    },
    Withdraw {
        gap: RecordId,
    },
    Score {
        hypothesis: RecordId,
        outcome: ScoreOutcome,
    },
}

impl VerdictAction {
    /// The complete state change this verdict effects: every record it moves,
    /// and the state it moves that record to. This is the single definition
    /// the state fold consumes — `touched()` is derived from it, so the index
    /// of affected records and the fold can never disagree.
    pub(crate) fn effects(&self) -> Vec<(RecordId, RecordState)> {
        match self {
            VerdictAction::Promote { target, retiring } => {
                let mut effects = vec![(*target, RecordState::Claim(ClaimState::Promoted))];
                if let Some(retiring) = retiring {
                    effects.push((*retiring, RecordState::Claim(ClaimState::Retired)));
                }
                effects
            }
            VerdictAction::Retire { target, .. } => {
                vec![(*target, RecordState::Claim(ClaimState::Retired))]
            }
            VerdictAction::Reject { target } => {
                vec![(*target, RecordState::Claim(ClaimState::Rejected))]
            }
            VerdictAction::Answer { gap, .. } => {
                vec![(*gap, RecordState::Gap(GapState::Answered))]
            }
            VerdictAction::Withdraw { gap } => {
                vec![(*gap, RecordState::Gap(GapState::Withdrawn))]
            }
            VerdictAction::Score { hypothesis, outcome } => {
                vec![(
                    *hypothesis,
                    RecordState::Hypothesis(HypothesisState::Scored(*outcome)),
                )]
            }
        }
    }

    /// The records whose state this action changes. Derived from `effects()`
    /// so the two cannot drift apart.
    pub(crate) fn touched(&self) -> Vec<RecordId> {
        self.effects().into_iter().map(|(id, _)| id).collect()
    }

    pub(crate) fn name(&self) -> &'static str {
        match self {
            VerdictAction::Promote { .. } => "promote",
            VerdictAction::Retire { .. } => "retire",
            VerdictAction::Reject { .. } => "reject",
            VerdictAction::Answer { .. } => "answer",
            VerdictAction::Withdraw { .. } => "withdraw",
            VerdictAction::Score { .. } => "score",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerdictContent {
    pub action: VerdictAction,
    pub rationale: Option<String>,
}

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
    Redaction,
}

impl fmt::Display for RecordKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            RecordKind::Claim => "claim",
            RecordKind::Gap => "gap",
            RecordKind::Hypothesis => "hypothesis",
            RecordKind::Redaction => "redaction",
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
    Redaction(RedactionContent),
}

impl Content {
    pub fn kind(&self) -> RecordKind {
        match self {
            Content::Claim(_) => RecordKind::Claim,
            Content::Gap(_) => RecordKind::Gap,
            Content::Hypothesis(_) => RecordKind::Hypothesis,
            Content::Verdict(_) => RecordKind::Verdict,
            Content::Redaction(_) => RecordKind::Redaction,
        }
    }
}

/// What stands where withheld text stood. A constant rather than a
/// convention, so a reader — human or test — matches one string and not a
/// family of near-misses.
pub const REDACTED: &str = "[redacted]";

/// A declared removal (U-11, D-0047). The declaration is an ordinary appended
/// record — human-authored, target checked, reason required — so the *fact*
/// of a redaction is as permanent as anything else in the log. The removal
/// itself happens later, in `redact_store`'s rewrite, which replaces the
/// withheld fields of the target's event and stamps the husk with this
/// record's id. Declaring without rewriting removes nothing, deliberately:
/// an append cannot un-write bytes, and pretending otherwise is how systems
/// leak while their logs say clean.
/// What a redaction is aimed at. Tagged on the wire because record and
/// entity ids serialize as bare ulids a reader cannot tell apart — and
/// widened from records-only (D-0047) to entities (D-0053) the day after it
/// shipped, while no durable store yet held a declaration to migrate.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum RedactionTarget {
    Record(RecordId),
    /// An entity has one redactable part — its label, where a person modeled
    /// as an entity would carry their name. Kind survives (it is structure),
    /// and the scope field is not consulted: a label is all there is.
    Entity(EntityId),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RedactionContent {
    pub target: RedactionTarget,
    pub scope: RedactionScope,
    /// Why this is being removed — the legal basis or the request's
    /// provenance. Required and non-empty: a removal with no stated ground is
    /// indistinguishable from tampering, which is the difference this whole
    /// mechanism exists to preserve.
    pub reason: String,
}

/// Which parts of the target's event the rewrite withholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RedactionScope {
    /// The author's name and detail. Kind survives — replay needs to know a
    /// human declared a verdict long after it stops knowing which human.
    Author,
    /// The record's prose: claim bodies, a gap's question, a hypothesis'
    /// statement, a verdict's rationale, evidence spans. Structure survives —
    /// entity references, verdict actions, and timestamps are what replay
    /// stands on.
    Content,
    /// The source reference — a URL or citation can carry a person's details
    /// as surely as a body can (U-46's second gap). The channel survives: it
    /// names a kind of provenance, not a person.
    Source,
    /// Everything above.
    Record,
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

/// Why a registered question left the register without being resolved.
///
/// The mirror of [`RetireReason`], and for the same reason: without it, "we
/// stopped asking" and "we asked it better" are the same recorded event, and a
/// keeper reading the ledger for drift cannot tell a tidy-up from a retreat.
/// Each variant means something different to that reader — a superseded
/// question is continuity, an answer held outside the ledger is a provenance
/// gap worth chasing, an irrelevant one marks the territory moving, and one
/// registered in error marks a mistake in the register rather than in the
/// world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum WithdrawReason {
    /// Asked again in a later record. The successor carries the link: this
    /// reason names the *shape* of the change, and `Envelope::supersedes` on
    /// the record that replaced it names the record.
    Superseded,
    /// The answer is known, and no record in this ledger states it. The
    /// loudest of the four: an answered question with no recorded answer is
    /// exactly the knowledge a keeper exists to capture.
    AnsweredElsewhere,
    /// Nothing answered it and nothing will — the question stopped mattering.
    NoLongerRelevant,
    /// It was never an open question: a duplicate, or already settled when it
    /// was written down.
    RegisteredInError,
    /// Recorded before this field existed. Read-only: [`Ledger::append`]
    /// refuses a verdict that states it, so it can arrive from an old log and
    /// can never be written. The alternative — defaulting old records to a
    /// real reason — would invent an account nobody gave.
    ///
    /// [`Ledger::append`]: crate::Ledger::append
    #[default]
    Unstated,
}

impl fmt::Display for WithdrawReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            WithdrawReason::Superseded => "superseded",
            WithdrawReason::AnsweredElsewhere => "answered elsewhere",
            WithdrawReason::NoLongerRelevant => "no longer relevant",
            WithdrawReason::RegisteredInError => "registered in error",
            WithdrawReason::Unstated => "unstated",
        };
        f.write_str(s)
    }
}

/// On what footing one person speaks for a whole set of records.
///
/// A set verdict does not weaken invariant 5 — a human still declares it, and
/// an agent still cannot. What it changes is what the declaration *means*, and
/// that difference has to be in the record rather than in someone's memory of
/// the afternoon. "I read each of these" and "I ratify the run that produced
/// them" are both honest, and a reader weighing a promoted claim needs to know
/// which one it rests on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SetBasis {
    /// Produced mechanically from one source by one run. The person ratifies
    /// the run and its source, not the records one at a time — which is the
    /// only honest thing to say about ten thousand rows of a catalogue.
    Ingestion,
    /// One editorial act that the keeper had to split across several records:
    /// a claim and its title, a claim and the cross-references read out of it.
    /// The person performed one act and the record should say one.
    OneAct,
    /// Read in full, record by record. The strong reading, available when it
    /// is true — and worth distinguishing precisely because the other two are
    /// weaker.
    ReviewedInFull,
}

impl fmt::Display for SetBasis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            SetBasis::Ingestion => "an ingestion run",
            SetBasis::OneAct => "one editorial act",
            SetBasis::ReviewedInFull => "reviewed in full",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VerdictAction {
    /// One verdict may promote a superseding claim and retire the record it
    /// supersedes — one decision, both transitions (design/001 §3.1).
    Promote {
        target: RecordId,
        retiring: Option<RecordId>,
    },
    /// One verdict over many claims, on a stated footing.
    ///
    /// The targets are enumerated rather than named by a run identifier, and
    /// that is forced rather than chosen: `VerdictAction::effects` is a pure
    /// function of the action, so a verdict that could not say what it touched
    /// without consulting the ledger would take the state fold's independence
    /// with it. The record is larger and says exactly what it did.
    PromoteSet {
        targets: Vec<RecordId>,
        /// Claims these replace, retired by the same verdict. Which replaces
        /// which is recorded by each record's own `supersedes` link, where
        /// supersession lives (D-0023).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        retiring: Vec<RecordId>,
        basis: SetBasis,
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
    /// A registered question leaves the register unanswered.
    Withdraw {
        gap: RecordId,
        /// Defaulted on read, never on write: logs written before reasons
        /// existed load as [`WithdrawReason::Unstated`] rather than being
        /// assigned a meaning after the fact.
        #[serde(default)]
        reason: WithdrawReason,
    },
    /// The hypothesis equivalent of [`VerdictAction::Withdraw`]: a dated
    /// prediction the project stops making before its score date. Separate
    /// from `Withdraw` because `VerdictAction::effects` is a pure function
    /// of the action — it must know the resulting state without consulting the
    /// ledger, and gaps and hypotheses do not share one.
    Abandon {
        hypothesis: RecordId,
        reason: WithdrawReason,
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
            VerdictAction::PromoteSet { targets, retiring, .. } => targets
                .iter()
                .map(|id| (*id, RecordState::Claim(ClaimState::Promoted)))
                .chain(
                    retiring.iter().map(|id| (*id, RecordState::Claim(ClaimState::Retired))),
                )
                .collect(),
            VerdictAction::Retire { target, .. } => {
                vec![(*target, RecordState::Claim(ClaimState::Retired))]
            }
            VerdictAction::Reject { target } => {
                vec![(*target, RecordState::Claim(ClaimState::Rejected))]
            }
            VerdictAction::Answer { gap, .. } => {
                vec![(*gap, RecordState::Gap(GapState::Answered))]
            }
            VerdictAction::Withdraw { gap, .. } => {
                vec![(*gap, RecordState::Gap(GapState::Withdrawn))]
            }
            VerdictAction::Abandon { hypothesis, .. } => {
                vec![(*hypothesis, RecordState::Hypothesis(HypothesisState::Abandoned))]
            }
            VerdictAction::Score { hypothesis, outcome } => {
                vec![(
                    *hypothesis,
                    RecordState::Hypothesis(HypothesisState::Scored(*outcome)),
                )]
            }
        }
    }

    /// Whether this verdict is what put `id` into the promoted set.
    ///
    /// Derived from `effects()` rather than matched on the variants, so a
    /// verdict added later is covered the day it is added. Adding
    /// `PromoteSet` made two audits go quietly blind — both asked "is this a
    /// `Promote` naming my claim?" and a set verdict is not — and an audit that
    /// reports nothing wrong because it stopped looking is the worst failure
    /// this record has a word for.
    pub fn promotes(&self, id: RecordId) -> bool {
        self.effects()
            .iter()
            .any(|(target, state)| *target == id && *state == RecordState::Claim(ClaimState::Promoted))
    }

    /// The records whose state this action changes. Derived from `effects()`
    /// so the two cannot drift apart.
    pub(crate) fn touched(&self) -> Vec<RecordId> {
        self.effects().into_iter().map(|(id, _)| id).collect()
    }

    pub(crate) fn name(&self) -> &'static str {
        match self {
            VerdictAction::Promote { .. } => "promote",
            VerdictAction::PromoteSet { .. } => "promote a set",
            VerdictAction::Retire { .. } => "retire",
            VerdictAction::Reject { .. } => "reject",
            VerdictAction::Answer { .. } => "answer",
            VerdictAction::Withdraw { .. } => "withdraw",
            VerdictAction::Abandon { .. } => "abandon",
            VerdictAction::Score { .. } => "score",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerdictContent {
    pub action: VerdictAction,
    pub rationale: Option<String>,
}

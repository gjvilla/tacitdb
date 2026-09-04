//! The shapes tools return.
//!
//! Every record an agent sees arrives with its envelope attached: author and
//! author kind, source channel, validity, lifecycle state, and its evidence
//! chain. Provenance is not an extra call — an answer that cannot say where it
//! came from is not an answer this engine gives.

use serde::Serialize;
use tacit_core::{ClaimContent, Content, Ledger, Record, RecordState, indexable_text};

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct EvidenceOut {
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<String>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct RecordOut {
    pub id: String,
    pub kind: String,
    pub state: String,
    pub text: String,
    pub author: String,
    pub author_kind: String,
    /// How the author is known. On a verdict transcribed from a document, what
    /// git could establish about who wrote the words asserting it — so an agent
    /// reading this can tell a promotion backed by a signed commit from one
    /// backed by nothing (U-29).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_known_by: Option<String>,
    pub source_channel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_reference: Option<String>,
    pub valid_from: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<String>,
    pub recorded_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_trigger: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<EvidenceOut>,
    /// Labels of the entities this record is about.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub about: Vec<String>,
    /// Present when part of this record was withheld by a declared redaction
    /// (U-11): the id of the redaction record that ordered it, whose reason
    /// states the ground. Absent means nothing was ever removed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redacted_by: Option<String>,
}

impl RecordOut {
    pub fn of(ledger: &Ledger, record: &Record) -> Self {
        let envelope = record.envelope();
        let entities = match record.content() {
            Content::Claim(claim) => claim.entity_refs(),
            Content::Gap(gap) => gap.territory.clone(),
            _ => Vec::new(),
        };
        Self {
            id: record.id().to_string(),
            kind: record.kind().to_string(),
            state: ledger
                .state_of(record.id())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "unknown".into()),
            text: display_text(record),
            author: envelope.author().name.clone(),
            author_kind: format!("{:?}", envelope.author().kind).to_lowercase(),
            author_known_by: envelope.author().detail.clone(),
            source_channel: envelope.source().channel.clone(),
            source_reference: envelope.source().reference.clone(),
            valid_from: envelope.valid_from().to_string(),
            valid_to: envelope.valid_to().map(|t| t.to_string()),
            recorded_at: envelope.recorded_at().to_string(),
            review_trigger: envelope.review_trigger().and_then(|t| {
                t.on_event.clone().or_else(|| t.due_at.map(|d| d.to_string()))
            }),
            evidence: envelope
                .evidence()
                .iter()
                .map(|e| EvidenceOut {
                    source: ledger
                        .entity(e.source)
                        .map(|s| s.label().to_string())
                        .unwrap_or_else(|| e.source.to_string()),
                    span: e.span.clone(),
                })
                .collect(),
            about: entities
                .iter()
                .filter_map(|e| ledger.entity(*e))
                .map(|e| format!("{}:{}", e.kind(), e.label()))
                .collect(),
            redacted_by: record.envelope().redacted().map(|mark| mark.by.to_string()),
        }
    }
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct VerdictOut {
    pub id: String,
    pub action: String,
    pub author: String,
    pub author_kind: String,
    pub at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
}

impl VerdictOut {
    pub fn of(record: &Record) -> Option<Self> {
        let Content::Verdict(verdict) = record.content() else { return None };
        let envelope = record.envelope();
        Some(Self {
            id: record.id().to_string(),
            action: describe_action(&verdict.action),
            author: envelope.author().name.clone(),
            author_kind: format!("{:?}", envelope.author().kind).to_lowercase(),
            at: envelope.recorded_at().to_string(),
            rationale: verdict.rationale.clone(),
        })
    }
}

fn describe_action(action: &tacit_core::VerdictAction) -> String {
    use tacit_core::VerdictAction as A;
    match action {
        A::Promote { target, retiring } => match retiring {
            Some(old) => format!("promote {target} (retiring {old})"),
            None => format!("promote {target}"),
        },
        A::PromoteSet { targets, retiring, basis } => format!(
            "promote {} claim(s) on the basis of {basis}{}",
            targets.len(),
            if retiring.is_empty() {
                String::new()
            } else {
                format!(", retiring {}", retiring.len())
            }
        ),
        A::Retire { target, reason } => format!("retire {target} ({reason:?})"),
        A::Reject { target } => format!("reject {target}"),
        A::Answer { gap, with_claim } => format!("answer {gap} with {with_claim}"),
        A::Withdraw { gap, reason } => format!("withdraw {gap} ({reason})"),
        A::Abandon { hypothesis, reason } => format!("abandon {hypothesis} ({reason})"),
        A::Score { hypothesis, outcome } => format!("score {hypothesis} {outcome:?}"),
    }
}

pub fn state_label(state: Option<RecordState>) -> String {
    state.map(|s| s.to_string()).unwrap_or_else(|| "not in the record".into())
}

/// What a client reads as the record's text. The index's text is the right
/// input for ranking and the wrong output for a reader: a decision's title,
/// forces and assertion joined by single spaces read as one run-on sentence,
/// which is what the first client saw. Here the assertion comes first, since
/// it is the claim, and the parts are separated; the tokens are unchanged, so
/// nothing about ranking or the suites moves. An excerpt, when the budget
/// assembled one, replaces this downstream as before.
pub fn display_text(record: &Record) -> String {
    match record.content() {
        Content::Claim(ClaimContent::Pattern { context, forces, solution, .. }) => {
            let mut text = solution.trim().to_string();
            if !context.trim().is_empty() {
                text.push_str("\n\nContext: ");
                text.push_str(context.trim());
            }
            if !forces.is_empty() {
                text.push_str("\n\nForces: ");
                text.push_str(&forces.iter().map(|f| f.trim()).collect::<Vec<_>>().join("; "));
            }
            text
        }
        _ => indexable_text(record).unwrap_or_default(),
    }
}

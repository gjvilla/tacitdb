//! A person's verdict, rendered at the keyboard.
//!
//! Until D-0055 the only way a verdict entered a store was transcription: a
//! person wrote `state: promoted` into a decision record and the ingest
//! carried it across, with what git could establish about the words attached
//! (D-0025). That path is right for decisions and useless for the other half
//! of the ratchet — an agent's proposal has no document to be promoted in,
//! so it waited in the inbox for a verdict nothing could render. This module
//! is that verdict. It draws no new grammar: the actions are the ledger's own
//! (`Promote`, `Reject`, `Retire`), the append runs the same checks a
//! transcribed verdict runs, and the tool surface still has no promote tool.
//!
//! `Withdraw` joined them on 2026-11-14 for the same reason seen from the
//! other record kind: an agent's registered question has no register row to
//! be withdrawn in, so a duplicate an agent registered stayed open until a
//! person could say at the keyboard that it was never a separate question.
//!
//! What it says about identity is the honest minimum. The name is asserted
//! at the keyboard, and the verdict's author detail records exactly that —
//! as an [`Attestation::None`] with its reason, in the same vocabulary the
//! ingest uses — so `review_trust` files it under "nothing to recheck"
//! rather than losing it, and "which promotions rest on a name someone
//! typed" stays a question the record can answer. A signed rung for
//! keyboard verdicts is not built; when it is wanted, this is where it goes.

use crate::attest::Attestation;
use tacit_core::{
    Author, AuthorKind, Content, Draft, Error, Ledger, RecordId, RetireReason, SourceRef,
    VerdictAction, VerdictContent, WithdrawReason,
};

/// The envelope channel every keyboard verdict carries.
pub const CHANNEL: &str = "tacit-keeper verdict";

/// The author detail's reason: what was, and was not, established.
pub const ASSERTED: &str = "name asserted at the keyboard to tacit-keeper; nothing verified";

/// What a person can rule from the keyboard. Deliberately the single-target
/// actions: a set verdict (D-0034) is an editorial act over an enumerated
/// list, and enumerating it on a command line is how the wrong id gets in.
#[derive(Debug, Clone, PartialEq)]
pub enum Ruling {
    Promote { target: RecordId, retiring: Option<RecordId> },
    Reject { target: RecordId },
    Retire { target: RecordId, reason: RetireReason },
    /// A registered open question leaves the register unanswered. The one
    /// ruling here whose target is a gap rather than a claim.
    Withdraw { gap: RecordId, reason: WithdrawReason },
}

impl Ruling {
    pub fn target(&self) -> RecordId {
        match self {
            Ruling::Promote { target, .. } | Ruling::Reject { target } | Ruling::Retire { target, .. } => {
                *target
            }
            Ruling::Withdraw { gap, .. } => *gap,
        }
    }

    fn action(&self) -> VerdictAction {
        match self.clone() {
            Ruling::Promote { target, retiring } => VerdictAction::Promote { target, retiring },
            Ruling::Reject { target } => VerdictAction::Reject { target },
            Ruling::Retire { target, reason } => VerdictAction::Retire { target, reason },
            Ruling::Withdraw { gap, reason } => VerdictAction::Withdraw { gap, reason },
        }
    }
}

/// The author a keyboard verdict carries: human, named as typed, and saying so.
pub fn author(who: &str) -> Author {
    Author {
        name: who.trim().to_string(),
        kind: AuthorKind::Human,
        detail: Some(Attestation::None { because: ASSERTED.to_string() }.to_string()),
    }
}

/// Append the verdict. The ledger's own grammar decides whether it is legal
/// — a promote of something already promoted, a retire of a proposal, an id
/// that is not a claim — and its refusal is returned as it was given.
pub fn render(ledger: &mut Ledger, who: &str, why: &str, ruling: &Ruling) -> Result<RecordId, Error> {
    let draft = Draft::new(
        author(who),
        SourceRef::channel(CHANNEL),
        Content::Verdict(VerdictContent {
            action: ruling.action(),
            rationale: Some(why.trim().to_string()),
        }),
    );
    ledger.append(draft)
}

/// The three reasons a claim may be retired, as a person types them.
pub fn retire_reason(text: &str) -> Option<RetireReason> {
    match text.trim().to_ascii_lowercase().as_str() {
        "superseded" => Some(RetireReason::Superseded),
        "no-longer-true" | "no_longer_true" => Some(RetireReason::NoLongerTrue),
        "promoted-in-error" | "promoted_in_error" => Some(RetireReason::PromotedInError),
        _ => None,
    }
}

/// The four reasons a question may be withdrawn, as a person types them.
/// `Unstated` is not among them: it is what a log older than reasons reads
/// as, and the ledger refuses a verdict that claims it.
pub fn withdraw_reason(text: &str) -> Option<WithdrawReason> {
    match text.trim().to_ascii_lowercase().as_str() {
        "superseded" => Some(WithdrawReason::Superseded),
        "answered-elsewhere" | "answered_elsewhere" => Some(WithdrawReason::AnsweredElsewhere),
        "no-longer-relevant" | "no_longer_relevant" => Some(WithdrawReason::NoLongerRelevant),
        "registered-in-error" | "registered_in_error" => Some(WithdrawReason::RegisteredInError),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tacit_core::{ClaimContent, ClaimState, GapContent, GapState, RecordState};

    fn question(ledger: &mut Ledger, text: &str) -> RecordId {
        ledger
            .append(Draft::new(
                Author::agent("latency-bot"),
                SourceRef::channel("test"),
                Content::Gap(GapContent { question: text.into(), territory: vec![] }),
            ))
            .unwrap()
    }

    #[test]
    fn a_keyboard_withdrawal_closes_a_question_and_keeps_its_reason() {
        let mut ledger = Ledger::new();
        let gap = question(&mut ledger, "is p99 measured at the edge or the origin?");
        assert_eq!(ledger.registered_gaps().len(), 1);

        let verdict = render(
            &mut ledger,
            "Jordan Lee",
            "duplicates U-11",
            &Ruling::Withdraw { gap, reason: WithdrawReason::RegisteredInError },
        )
        .unwrap();
        assert_eq!(ledger.state_of(gap), Some(RecordState::Gap(GapState::Withdrawn)));
        assert!(ledger.registered_gaps().is_empty());

        let record = ledger.record(verdict).unwrap();
        assert_eq!(record.envelope().author().kind, AuthorKind::Human);
        let Content::Verdict(content) = record.content() else { panic!("a verdict") };
        assert_eq!(content.rationale.as_deref(), Some("duplicates U-11"));
        assert!(matches!(
            content.action,
            VerdictAction::Withdraw { reason: WithdrawReason::RegisteredInError, .. }
        ));
    }

    #[test]
    fn a_withdrawal_is_refused_for_a_claim_and_for_a_question_already_closed() {
        let mut ledger = Ledger::new();
        let claim = proposal(&mut ledger, "a");
        let on_a_claim = render(
            &mut ledger,
            "J",
            "x",
            &Ruling::Withdraw { gap: claim, reason: WithdrawReason::NoLongerRelevant },
        );
        assert!(on_a_claim.is_err(), "{on_a_claim:?}");
        assert_eq!(ledger.state_of(claim), Some(RecordState::Claim(ClaimState::Proposed)));

        let gap = question(&mut ledger, "q?");
        let ruling = Ruling::Withdraw { gap, reason: WithdrawReason::Superseded };
        render(&mut ledger, "J", "asked better elsewhere", &ruling).unwrap();
        let twice = render(&mut ledger, "J", "again", &ruling);
        assert!(matches!(twice, Err(Error::IllegalTransition { .. })), "{twice:?}");

        let unstated =
            render(&mut ledger, "J", "x", &Ruling::Withdraw { gap, reason: WithdrawReason::Unstated });
        assert!(matches!(unstated, Err(Error::UnstatedWithdrawReason)), "{unstated:?}");
    }

    #[test]
    fn withdraw_reasons_are_the_four_a_verdict_may_state() {
        assert_eq!(withdraw_reason("superseded"), Some(WithdrawReason::Superseded));
        assert_eq!(withdraw_reason("Answered-Elsewhere"), Some(WithdrawReason::AnsweredElsewhere));
        assert_eq!(withdraw_reason("no_longer_relevant"), Some(WithdrawReason::NoLongerRelevant));
        assert_eq!(withdraw_reason("registered-in-error"), Some(WithdrawReason::RegisteredInError));
        assert_eq!(withdraw_reason("unstated"), None);
        assert_eq!(withdraw_reason("because"), None);
    }

    fn proposal(ledger: &mut Ledger, body: &str) -> RecordId {
        ledger
            .append(Draft::new(
                Author::agent("latency-bot"),
                SourceRef::channel("test"),
                Content::Claim(ClaimContent::Text { body: body.into(), about: vec![] }),
            ))
            .unwrap()
    }

    #[test]
    fn a_keyboard_promote_moves_a_proposal_and_says_how_it_was_identified() {
        let mut ledger = Ledger::new();
        let claim = proposal(&mut ledger, "p99 is 2.4s");
        assert_eq!(ledger.state_of(claim), Some(RecordState::Claim(ClaimState::Proposed)));

        let verdict = render(
            &mut ledger,
            "Jordan Lee",
            "measured twice",
            &Ruling::Promote { target: claim, retiring: None },
        )
        .unwrap();
        assert_eq!(ledger.state_of(claim), Some(RecordState::Claim(ClaimState::Promoted)));

        let author = ledger.record(verdict).unwrap().envelope().author().clone();
        assert_eq!(author.kind, AuthorKind::Human);
        assert_eq!(author.name, "Jordan Lee");
        let parsed = Attestation::parse(author.detail.as_deref().unwrap()).expect("readable");
        assert!(matches!(parsed, Attestation::None { because } if because == ASSERTED));
    }

    #[test]
    fn reject_and_retire_run_the_same_grammar() {
        let mut ledger = Ledger::new();
        let a = proposal(&mut ledger, "a");
        let b = proposal(&mut ledger, "b");
        render(&mut ledger, "J", "no", &Ruling::Reject { target: a }).unwrap();
        assert_eq!(ledger.state_of(a), Some(RecordState::Claim(ClaimState::Rejected)));

        render(&mut ledger, "J", "yes", &Ruling::Promote { target: b, retiring: None }).unwrap();
        render(
            &mut ledger,
            "J",
            "oops",
            &Ruling::Retire { target: b, reason: RetireReason::PromotedInError },
        )
        .unwrap();
        assert_eq!(ledger.state_of(b), Some(RecordState::Claim(ClaimState::Retired)));
    }

    #[test]
    fn the_grammar_refuses_what_it_always_refused() {
        let mut ledger = Ledger::new();
        let a = proposal(&mut ledger, "a");
        render(&mut ledger, "J", "yes", &Ruling::Promote { target: a, retiring: None }).unwrap();
        let again = render(&mut ledger, "J", "again", &Ruling::Promote { target: a, retiring: None });
        assert!(matches!(again, Err(Error::IllegalTransition { .. })));
        let retire_a_proposal = {
            let c = proposal(&mut ledger, "c");
            render(&mut ledger, "J", "x", &Ruling::Retire { target: c, reason: RetireReason::Superseded })
        };
        assert!(matches!(retire_a_proposal, Err(Error::IllegalTransition { .. })));
    }

    #[test]
    fn a_keyboard_promotion_is_counted_by_the_trust_review_not_lost() {
        let mut ledger = Ledger::new();
        let a = proposal(&mut ledger, "a");
        render(&mut ledger, "J", "yes", &Ruling::Promote { target: a, retiring: None }).unwrap();
        let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let review = crate::review_trust(&ledger, &repo);
        assert_eq!(review.nothing_to_recheck.len(), 1, "{review:?}");
        assert!(review.weakened.is_empty());
    }

    #[test]
    fn retire_reasons_are_the_three_the_grammar_has() {
        assert_eq!(retire_reason("superseded"), Some(RetireReason::Superseded));
        assert_eq!(retire_reason("No-Longer-True"), Some(RetireReason::NoLongerTrue));
        assert_eq!(retire_reason("promoted_in_error"), Some(RetireReason::PromotedInError));
        assert_eq!(retire_reason("because"), None);
    }
}

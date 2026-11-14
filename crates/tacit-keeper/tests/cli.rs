//! The keeper binary's front door, checked against the real binary: help is
//! answered on stdout with exit 0, the way the host answers it, and a usage
//! mistake is explained on stderr with exit 2. The two were once different
//! and a stranger noticed.

use std::process::Command;

#[test]
fn help_answers_on_stdout_and_exits_clean() {
    for flag in ["--help", "-h"] {
        let out = Command::new(env!("CARGO_BIN_EXE_tacit-keeper")).arg(flag).output().unwrap();
        assert!(out.status.success(), "{flag} exits 0");
        let text = String::from_utf8(out.stdout).unwrap();
        for word in ["pending", "promote", "reject", "retire", "withdraw", "registered-in-error", "--store", "--as", "--why"] {
            assert!(text.contains(word), "help names {word}");
        }
        assert!(out.stderr.is_empty(), "help does not write to stderr");
    }
}

#[test]
fn a_usage_mistake_is_explained_on_stderr_and_exits_two() {
    let out = Command::new(env!("CARGO_BIN_EXE_tacit-keeper")).args(["promote"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8(out.stderr).unwrap();
    assert!(err.contains("--store is required"), "{err}");
    assert!(err.contains("Usage:"), "the usage follows the reason");
}

/// A scratch store holding one registered question and one proposed claim,
/// both written the way an agent's MCP session writes them.
fn store_with_a_question_and_a_claim(name: &str) -> (std::path::PathBuf, String, String) {
    use tacit_core::{Author, ClaimContent, Content, Draft, GapContent, Ledger, SourceRef};
    let dir = std::env::temp_dir().join(format!("tacit-keeper-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let store = dir.join("store.log");
    let mut ledger = Ledger::open(&store).unwrap().ledger;
    let source = || SourceRef { channel: "mcp".into(), reference: Some("cli test".into()) };
    let gap = ledger
        .append(Draft::new(
            Author::agent("test-agent"),
            source(),
            Content::Gap(GapContent {
                question: "who settles quokka husbandry rotas?".into(),
                territory: vec![],
            }),
        ))
        .unwrap();
    let claim = ledger
        .append(Draft::new(
            Author::agent("test-agent"),
            source(),
            Content::Claim(ClaimContent::Text { body: "rotas are settled weekly".into(), about: vec![] }),
        ))
        .unwrap();
    drop(ledger);
    (store, gap.to_string(), claim.to_string())
}

fn keeper(args: &[&str]) -> (Option<i32>, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_tacit-keeper")).args(args).output().unwrap();
    (
        out.status.code(),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

/// The withdrawal lands in the log, not just in the process that rendered
/// it: the store reopened afterwards holds the question as withdrawn, with
/// the reason and the rationale the person gave, and no longer lists it open.
#[test]
fn withdraw_closes_a_registered_question_in_the_store() {
    use tacit_core::{Content, GapState, Ledger, RecordState, VerdictAction, WithdrawReason};
    let (store, gap, _) = store_with_a_question_and_a_claim("withdraw");
    let path = store.to_str().unwrap();

    let (code, out, err) = keeper(&[
        "withdraw", "--store", path, "--as", "Jordan Lee", "--why", "duplicates U-11",
        "--reason", "registered-in-error", &gap,
    ]);
    assert_eq!(code, Some(0), "{err}");
    assert!(out.contains("Registered") && out.contains("Withdrawn"), "{out}");
    assert!(out.contains("Jordan Lee") && out.contains("duplicates U-11"), "{out}");

    let ledger = Ledger::open(&store).unwrap().ledger;
    let id = gap.parse().unwrap();
    assert_eq!(ledger.state_of(id), Some(RecordState::Gap(GapState::Withdrawn)));
    assert!(ledger.registered_gaps().is_empty());
    let history = ledger.history(id);
    let Content::Verdict(verdict) = history[0].content() else { panic!("a verdict") };
    assert!(matches!(
        verdict.action,
        VerdictAction::Withdraw { reason: WithdrawReason::RegisteredInError, .. }
    ));
    assert_eq!(verdict.rationale.as_deref(), Some("duplicates U-11"));
    assert_eq!(history[0].envelope().author().name, "Jordan Lee");
    drop(ledger);

    // A second withdrawal of the same question is the ledger's refusal.
    let (code, _, err) = keeper(&[
        "withdraw", "--store", path, "--as", "Jordan Lee", "--why", "again",
        "--reason", "superseded", &gap,
    ]);
    assert_eq!(code, Some(1));
    assert!(err.contains("the ledger refused"), "{err}");
}

#[test]
fn withdraw_refuses_what_is_not_a_registered_question() {
    use tacit_core::{ClaimState, Ledger, RecordState};
    let (store, _, claim) = store_with_a_question_and_a_claim("refuse");
    let path = store.to_str().unwrap();

    let (code, _, err) = keeper(&[
        "withdraw", "--store", path, "--as", "J", "--why", "x", "--reason", "superseded", &claim,
    ]);
    assert_eq!(code, Some(1));
    assert!(err.contains("the ledger refused"), "{err}");

    // An id the store has never held.
    let (code, _, err) = keeper(&[
        "withdraw", "--store", path, "--as", "J", "--why", "x", "--reason", "superseded",
        "rec_01M1P58RMFAD6DC86Q3D6JV6H2",
    ]);
    assert_eq!(code, Some(1), "{err}");

    let ledger = Ledger::open(&store).unwrap().ledger;
    assert_eq!(ledger.state_of(claim.parse().unwrap()), Some(RecordState::Claim(ClaimState::Proposed)));
    assert_eq!(ledger.registered_gaps().len(), 1, "nothing was written");
}

#[test]
fn withdraw_usage_mistakes_exit_two_and_write_nothing() {
    use tacit_core::Ledger;
    let (store, gap, _) = store_with_a_question_and_a_claim("usage");
    let path = store.to_str().unwrap();
    let cases: [(&[&str], &str); 7] = [
        (&["--as", "J", "--why", "x", &gap], "--reason is required for withdraw"),
        (&["--as", "J", "--why", "x", "--reason", "unstated", &gap], "unknown reason"),
        (&["--as", "J", "--why", "x", "--reason", "no-longer-true", &gap], "registered-in-error"),
        (&["--as", "J", "--reason", "superseded", &gap], "--why is required"),
        (&["--why", "x", "--reason", "superseded", &gap], "--as is required"),
        (&["--as", "J", "--why", "x", "--reason", "superseded"], "a record id is required"),
        (&["--as", "J", "--why", "x", "--reason", "superseded", "--retiring", &gap, &gap], "--retiring belongs to promote"),
    ];
    for (rest, expected) in cases {
        let mut args = vec!["withdraw", "--store", path];
        args.extend_from_slice(rest);
        let (code, _, err) = keeper(&args);
        assert_eq!(code, Some(2), "{rest:?}: {err}");
        assert!(err.contains(expected), "{rest:?}: {err}");
    }
    // --reason still does not belong to the commands that take none.
    let (code, _, err) = keeper(&["reject", "--store", path, "--as", "J", "--why", "x", "--reason", "superseded", &gap]);
    assert_eq!(code, Some(2));
    assert!(err.contains("--reason belongs to retire and withdraw"), "{err}");

    assert_eq!(Ledger::open(&store).unwrap().ledger.registered_gaps().len(), 1);
}

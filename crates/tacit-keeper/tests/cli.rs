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
        for word in ["pending", "promote", "reject", "retire", "--store", "--as", "--why"] {
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

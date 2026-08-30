//! Read a directory of proposals into a ledger and say what came of them.
//!
//! `cargo run -p tacit-keeper --example proposals -- <dir>`
//!
//! The directory is the caller's, not the repository's: no proposals are
//! vendored while U-11 is open (see `pep.rs`). Point it at a checkout and it
//! reports what the adapter understood, what it had to judge, and what it could
//! not resolve — which is how a slice of a corpus is supposed to answer.

use std::path::PathBuf;
use std::process::ExitCode;
use tacit_core::{Ledger, Projection, TextIndex};
use tacit_keeper::pep::{Pep, ingest_peps, parse_pep};

fn main() -> ExitCode {
    let Some(dir) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: proposals <dir of .rst files>");
        return ExitCode::FAILURE;
    };

    let mut files: Vec<PathBuf> = match std::fs::read_dir(&dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "rst" || e == "txt"))
            .collect(),
        Err(error) => {
            eprintln!("could not read {}: {error}", dir.display());
            return ExitCode::FAILURE;
        }
    };
    files.sort();

    let mut peps: Vec<Pep> = Vec::new();
    let mut refused: Vec<(String, String)> = Vec::new();
    for path in &files {
        let Ok(text) = std::fs::read_to_string(path) else { continue };
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        match parse_pep(&text) {
            Ok(pep) => peps.push(pep),
            // A document this adapter cannot read is reported by name. It is a
            // fact about the adapter's coverage, and counting it as zero would
            // be the silent drop the corpus parser refuses to make.
            Err(error) => refused.push((name, error.to_string())),
        }
    }
    peps.sort_by_key(|p| p.number);

    println!("\n\x1b[1mPROPOSALS\x1b[0m");
    println!("{}", "─".repeat(64));
    println!("  {} files, {} parsed, {} refused", files.len(), peps.len(), refused.len());
    for (name, error) in refused.iter().take(10) {
        println!("    REFUSED {name}: {error}");
    }
    if refused.len() > 10 {
        println!("    … and {} more", refused.len() - 10);
    }

    let mut ledger = Ledger::new();
    let report = match ingest_peps(&mut ledger, &peps) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("ingest failed: {error}");
            return ExitCode::FAILURE;
        }
    };

    println!("\n\x1b[1mLEDGER\x1b[0m");
    println!("{}", "─".repeat(64));
    println!(
        "  {} records over {} proposals, {} entities",
        report.records,
        report.proposals,
        ledger.entities().count()
    );
    println!(
        "    promoted {:>4}   proposed {:>4}   refused {:>4}   retired {:>4}",
        report.promoted, report.proposed, report.refused, report.retired
    );

    println!("\n\x1b[1mWHAT IT HAD TO DECIDE\x1b[0m");
    println!("{}", "─".repeat(64));
    println!(
        "  {} proposals rest on reading Provisional or Deferred, which this",
        report.judged.len()
    );
    println!("  engine has no state for. Named so the count is answerable:");
    for label in report.judged.iter().take(8) {
        println!("    {label}");
    }
    if report.judged.len() > 8 {
        println!("    … and {} more", report.judged.len() - 8);
    }
    println!("\n  {} links point outside the slice:", report.dangling.len());
    for (label, what) in report.dangling.iter().take(8) {
        println!("    {label}  {what}");
    }
    if report.dangling.len() > 8 {
        println!("    … and {} more", report.dangling.len() - 8);
    }

    let _projection = Projection::rebuild(&ledger);
    let index = TextIndex::rebuild(&ledger);
    println!("\n\x1b[1mINDEX\x1b[0m");
    println!("{}", "─".repeat(64));
    println!("  {} indexed documents", index.documents());
    println!();
    ExitCode::SUCCESS
}

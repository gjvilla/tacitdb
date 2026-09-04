//! The Tacit MCP host: one small binary that embeds the engine and speaks MCP
//! over stdio (D-0015). There is no wire protocol to the engine, no driver and
//! no connection pool — the library is in this process.

mod server;
mod shapes;
mod store;

use rmcp::ServiceExt;
use rmcp::transport::stdio;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use store::Store;
use tacit_core::Ledger;
use std::collections::BTreeSet;
use tacit_keeper::{Attest, Disposition};

/// What `--help` prints. Written to stdout, the one time that channel is not
/// MCP: nobody is on the other end of a process that exits before it serves,
/// and a shell expects help where a shell looks for it.
const USAGE: &str = "\
tacit-mcp — serve a decision-record corpus over MCP (stdio)

Usage: tacit-mcp [OPTIONS] [CORPUS]

Arguments:
  CORPUS                 A directory holding docs/DECISIONS.md and docs/REGISTER.md.
                         Both are read on every start as a sync: unchanged records
                         write nothing, edited ones supersede what they replace.
                         Without one the host serves an empty store.

Options:
  --store <PATH>         Keep the ledger on disk at PATH, with its audit log beside
                         it as PATH.audit. Without it the ledger dies at exit.
  --require-signature    Decline to transcribe a promotion whose words no signed
                         commit carries; the claim stays proposed. The default
                         records what git can establish and says so in the verdict.
  --signed-by <NAME>     Accept promotions only from NAME's verified signature.
                         Repeatable. Implies --require-signature.
  -h, --help             Print this and exit.

The tool surface has no promote tool. What an agent proposes waits for a person,
who promotes by writing the decision into docs/DECISIONS.md. See the README's
\"Your own corpus\" section for the document format.
";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // stdout is the MCP channel; everything human-facing goes to stderr.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut corpus = None;
    let mut store = None;
    let mut require_signature = false;
    let mut signers: BTreeSet<String> = BTreeSet::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--store" => {
                let Some(path) = args.get(index + 1) else {
                    eprintln!("tacit-mcp: --store needs a path");
                    std::process::exit(2);
                };
                store = Some(PathBuf::from(path));
                index += 2;
            }
            "--require-signature" => {
                require_signature = true;
                index += 1;
            }
            // Named here and never read out of the repository: a list of who
            // may promote, kept in the file it protects, is one more file an
            // agent can edit.
            "--signed-by" => {
                let Some(name) = args.get(index + 1) else {
                    eprintln!("tacit-mcp: --signed-by needs a signer's name");
                    std::process::exit(2);
                };
                signers.insert(name.clone());
                require_signature = true;
                index += 2;
            }
            "-h" | "--help" => {
                print!("{USAGE}");
                std::process::exit(0);
            }
            other if other.starts_with('-') => {
                eprintln!("tacit-mcp: unknown option {other} (try --help)");
                std::process::exit(2);
            }
            other => {
                corpus = Some(PathBuf::from(other));
                index += 1;
            }
        }
    }

    // With a store, the ledger outlives the process and what agents propose
    // is still waiting for a person next time. Without one it is a scratch
    // ledger that dies at exit — useful for a look around, useless as memory.
    let mut ledger = match &store {
        Some(path) => match Ledger::open(path) {
            Ok(opened) => {
                eprintln!(
                    "tacit-mcp: replayed {} events from {}{}",
                    opened.recovery.events_replayed,
                    path.display(),
                    if opened.recovery.truncated_bytes > 0 {
                        format!(
                            " (dropped {} bytes of a torn final write)",
                            opened.recovery.truncated_bytes
                        )
                    } else {
                        String::new()
                    }
                );
                // Said out loud rather than swallowed: it is the fact that
                // explains the next refused append (U-22).
                if let Some(ahead) = opened.recovery.leads_clock {
                    eprintln!(
                        "tacit-mcp: this machine's clock is {ahead} behind the log — record-time \
                         holds at the last entry until it catches up"
                    );
                }
                opened.ledger
            }
            Err(error) => {
                eprintln!("tacit-mcp: could not open {}: {error}", path.display());
                std::process::exit(1);
            }
        },
        None => Ledger::new(),
    };

    let attest = match (require_signature, signers.is_empty()) {
        (false, _) => Attest::Observe,
        (true, true) => Attest::RequireSignature,
        (true, false) => Attest::RequireSignatureFrom(signers),
    };

    // The documents are upstream and the store is downstream, so every start
    // is a sync: unchanged records write nothing, edited ones supersede what
    // they replace, and what the ingest may not decide on its own is said out
    // loud (U-19).
    match &corpus {
        Some(root) => match tacit_keeper::ingest_corpus_with(&mut ledger, root, attest) {
            Ok(report) => {
                if report.unreadable_provenance {
                    eprintln!(
                        "tacit-mcp: this store holds records whose provenance this build \
                         cannot read — a store written before D-0021. Everything below \
                         landed as new, so the corpus is now in it twice. Start a fresh \
                         store rather than living with the duplicate."
                    );
                }
                eprintln!(
                    "tacit-mcp: synced {} source records from {}: {} new, {} edited, {} \
                     unchanged ({} records written)",
                    report.dispositions.len(),
                    root.display(),
                    report.count(Disposition::Fresh),
                    report.count(Disposition::Changed),
                    report.count(Disposition::Unchanged),
                    report.appended(),
                );
                // Three things the sync reports and will not act on, because
                // each of them is a verdict and verdicts are human acts.
                for id in &report.absent {
                    eprintln!(
                        "tacit-mcp:   {id} is in the store and gone from the document — \
                         it stays as it is; retiring it is a person's verdict"
                    );
                }
                for id in &report.drifted {
                    eprintln!(
                        "tacit-mcp:   {id} was reworded after it was settled — left as it \
                         is, because that is history and history is not rewritten"
                    );
                }
                for (id, state) in &report.refused {
                    eprintln!(
                        "tacit-mcp:   {id} reads `state: promoted` in the document and \
                         {state} in the store — not resurrected"
                    );
                }
                // The whole of U-29 in one line each: a verdict transcribed
                // from prose is only as good as what is known about who wrote
                // the prose.
                for (id, why) in &report.withheld {
                    eprintln!(
                        "tacit-mcp:   {id} asserts a verdict this run will not carry: {why} \
                         — not transcribed; the claim stays proposed"
                    );
                }
                if !report.unattested.is_empty() {
                    eprintln!(
                        "tacit-mcp:   {} verdict(s) transcribed with nothing established \
                         about who wrote them: {}. Each says so in its own author detail. \
                         Pass --require-signature to decline them instead.",
                        report.unattested.len(),
                        report.unattested.join(", ")
                    );
                }
            }
            Err(error) => {
                eprintln!("tacit-mcp: could not load corpus from {}: {error}", root.display());
                std::process::exit(1);
            }
        },
        None => eprintln!(
            "tacit-mcp: starting with an empty store. Pass a repository path to load its \
             docs/DECISIONS.md and docs/REGISTER.md."
        ),
    }

    // A weakened attestation is a drift alarm, and an alarm nobody is standing
    // in front of is not an alarm. One git call, on the way up (U-32).
    if let Some(root) = &corpus {
        let review = tacit_keeper::review_trust(&ledger, root);
        if !review.quiet() {
            eprintln!(
                "tacit-mcp: {} promotion(s) rest on a signature that no longer verifies as \
                 it did when the verdict was made:",
                review.weakened.len()
            );
            const SHOWN: usize = 5;
            for row in review.weakened.iter().take(SHOWN) {
                let now = match &row.today {
                    tacit_keeper::Verified::Changed(now) => now.to_string(),
                    other => format!("{other:?}"),
                };
                eprintln!("tacit-mcp:   {} was {} and is now {now}", row.claim, row.recorded);
            }
            // Said rather than silently swallowed: a truncated list that does
            // not say it was truncated reads as a complete one.
            if let Some(rest) = review.weakened.len().checked_sub(SHOWN).filter(|n| *n > 0) {
                eprintln!("tacit-mcp:   ... and {rest} more, all of them still in the record");
            }
            eprintln!(
                "tacit-mcp:   Nothing has been changed in the record. A key that stopped \
                 being trusted is not a verdict, and retiring what it signed is a person's \
                 to declare."
            );
        }
    }

    let durable = ledger.journal_path().map(|p| p.display().to_string());
    // The audit lives beside the store when there is one: usage of a durable
    // corpus is worth observing (U-3's trigger is exactly that observation),
    // and a scratch ledger's usage dies with it, consistently.
    let state = match &store {
        Some(path) => {
            let audit = path.with_extension("audit");
            Arc::new(Mutex::new(Store::new(ledger).with_audit(audit)))
        }
        None => Arc::new(Mutex::new(Store::new(ledger))),
    };
    eprintln!(
        "tacit-mcp: serving {} records on stdio ({})",
        state.lock().expect("store lock").ledger.log().len(),
        durable.map(|p| format!("durable: {p}")).unwrap_or_else(|| "in memory only".into())
    );

    let service = server::TacitServer::new(state).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

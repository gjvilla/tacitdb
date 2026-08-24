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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // stdout is the MCP channel; everything human-facing goes to stderr.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut corpus = None;
    let mut store = None;
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
            other if other.starts_with('-') => {
                eprintln!("tacit-mcp: unknown option {other}");
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
                opened.ledger
            }
            Err(error) => {
                eprintln!("tacit-mcp: could not open {}: {error}", path.display());
                std::process::exit(1);
            }
        },
        None => Ledger::new(),
    };

    // Only load the corpus into a store that does not already hold it;
    // re-ingesting would duplicate every record (U-19).
    let already_loaded = !ledger.log().is_empty();
    match &corpus {
        Some(_) if already_loaded => eprintln!(
            "tacit-mcp: the store already holds {} records; not re-ingesting the corpus",
            ledger.log().len()
        ),
        Some(root) => match tacit_keeper::ingest_corpus(&mut ledger, root) {
            Ok(report) => eprintln!(
                "tacit-mcp: loaded {} decision records and {} register entries from {}",
                report.decisions.len(),
                report.gaps.len(),
                root.display()
            ),
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

    let durable = ledger.journal_path().map(|p| p.display().to_string());
    let state = Arc::new(Mutex::new(Store::new(ledger)));
    eprintln!(
        "tacit-mcp: serving {} records on stdio ({})",
        state.lock().expect("store lock").ledger.log().len(),
        durable.map(|p| format!("durable: {p}")).unwrap_or_else(|| "in memory only".into())
    );

    let service = server::TacitServer::new(state).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

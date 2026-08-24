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
use tacit_core::MemoryLedger;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // stdout is the MCP channel; everything human-facing goes to stderr.
    let corpus = std::env::args().skip(1).find(|a| !a.starts_with('-')).map(PathBuf::from);

    let mut ledger = MemoryLedger::new();
    match &corpus {
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

    let store = Arc::new(Mutex::new(Store::new(ledger)));
    eprintln!(
        "tacit-mcp: serving {} records on stdio",
        store.lock().expect("store lock").ledger.log().len()
    );

    let service = server::TacitServer::new(store).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

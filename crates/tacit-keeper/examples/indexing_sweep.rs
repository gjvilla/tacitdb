//! What a passage size costs, measured on both suites at once — U-39's
//! instrument, built to the same rule as fusion_sweep: a change to how
//! records are indexed is believed only when it has been graded over both
//! corpora, with the status quo in the table as the `whole` row rather than
//! remembered.
//!
//! `cargo run --release -p tacit-keeper --example indexing_sweep`

use std::path::PathBuf;
use tacit_core::{Embedder, HashingEmbedder, Ledger, Projection, TextIndex, VectorIndex};
use tacit_keeper::corpus::ingest_corpus;
use tacit_keeper::golden::{GoldenQuestion, Scorecard, parse_golden, parse_golden_rows, run_with};
use tacit_keeper::pep::{ingest_peps, parse_pep};

struct Corpus {
    name: &'static str,
    ledger: Ledger,
    questions: Vec<GoldenQuestion>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");

    let mut corpora: Vec<Corpus> = Vec::new();
    {
        let mut ledger = Ledger::new();
        ingest_corpus(&mut ledger, &repo)?;
        let questions = parse_golden(&std::fs::read_to_string(repo.join("docs/GOLDEN.md"))?)?;
        corpora.push(Corpus { name: "self-hosting", ledger, questions });
    }
    let dir = repo.join("target/proposals");
    if dir.is_dir() {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "rst" || e == "txt"))
            .collect();
        files.sort();
        let mut peps = Vec::new();
        for path in &files {
            peps.push(parse_pep(&std::fs::read_to_string(path)?)?);
        }
        peps.sort_by_key(|p| p.number);
        let mut ledger = Ledger::new();
        ingest_peps(&mut ledger, &peps)?;
        let questions =
            parse_golden_rows(&std::fs::read_to_string(repo.join("docs/PEP-GOLDEN.md"))?, "P-")?;
        corpora.push(Corpus { name: "proposals", ledger, questions });
    } else {
        println!("(no corpus at {} — self-hosting suite only)", dir.display());
    }

    let sizes: Vec<(String, usize)> = [100usize, 200, 300, 400, 600, 800]
        .iter()
        .map(|n| (format!("passage {n}"), *n))
        .chain([("whole record".to_string(), usize::MAX)])
        .collect();

    for corpus in &corpora {
        let projection = Projection::rebuild(&corpus.ledger);
        let embedder = HashingEmbedder::default();
        let vectors = VectorIndex::rebuild(&corpus.ledger, &embedder);

        // The baseline every row is compared against is the shipped default.
        let shipped = TextIndex::rebuild(&corpus.ledger);
        let base = run_with(
            &corpus.ledger,
            &projection,
            &shipped,
            Some((&vectors, &embedder as &dyn Embedder)),
            &corpus.questions,
        );

        println!(
            "\n\x1b[1m{}\x1b[0m — {} questions, shipped default passes {}",
            corpus.name.to_uppercase(),
            corpus.questions.len(),
            base.passed()
        );
        for (label, size) in &sizes {
            let mut index = TextIndex::empty().with_passage_tokens(*size);
            index.advance(&corpus.ledger);
            let card = run_with(
                &corpus.ledger,
                &projection,
                &index,
                Some((&vectors, &embedder as &dyn Embedder)),
                &corpus.questions,
            );
            println!(
                "  {label:<13} {:>4} docs  {:>2}/{} passed   {}",
                index.documents(),
                card.passed(),
                card.graded.len(),
                flips(&base, &card)
            );
        }
    }
    Ok(())
}

/// What moved relative to the shipped default, id by id.
fn flips(shipped: &Scorecard, card: &Scorecard) -> String {
    let moved: Vec<String> = shipped
        .graded
        .iter()
        .zip(&card.graded)
        .filter(|(a, b)| a.verdict != b.verdict)
        .map(|(a, b)| format!("{} {}->{}", a.question.id, a.verdict.label(), b.verdict.label()))
        .collect();
    if moved.is_empty() { "—".to_string() } else { moved.join("  ") }
}

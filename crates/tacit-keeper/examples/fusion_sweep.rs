//! What a fusion plan costs, measured on both suites at once.
//!
//! `cargo run --release -p tacit-keeper --example fusion_sweep`
//!
//! U-41's rule, kept after its resolution (D-0040): a fusion change is
//! believed only when it has been graded over both corpora. The k=60 default
//! this instrument retired was measured *earning* four questions on the
//! proposals corpus while *costing* two on the self-hosting one — and a sweep
//! over one suite is how D-0028's "the constant does not matter" got
//! recorded, true of that corpus and false in general.
//!
//! The proposals corpus is graded when `target/proposals` holds the pinned
//! slice (scripts/fetch-proposals.sh); otherwise the sweep says so and runs
//! what it has.

use std::path::PathBuf;
use tacit_core::{
    Embedder, Fusion, HashingEmbedder, Ledger, Projection, TextIndex, VectorIndex,
};
use tacit_keeper::corpus::ingest_corpus;
use tacit_keeper::golden::{GoldenQuestion, Scorecard, parse_golden, parse_golden_rows, run_fused, run_with};
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
        println!("(no corpus at {} — sweeping the self-hosting suite only)", dir.display());
    }

    let plans: Vec<(String, Fusion)> = [60.0, 30.0, 10.0, 5.0, 2.0, 1.0, 0.0]
        .iter()
        .map(|k| (format!("rrf k={k}"), Fusion::Rrf { k: *k }))
        .chain([("weighted 1:1".to_string(), Fusion::Weighted(vec![1.0, 1.0]))])
        .collect();

    for corpus in &corpora {
        let projection = Projection::rebuild(&corpus.ledger);
        let index = TextIndex::rebuild(&corpus.ledger);
        let embedder = HashingEmbedder::default();
        let vectors = VectorIndex::rebuild(&corpus.ledger, &embedder);
        let with = Some((&vectors, &embedder as &dyn Embedder));

        let lexical = run_with(&corpus.ledger, &projection, &index, None, &corpus.questions);
        let shipped = run_fused(
            &corpus.ledger,
            &projection,
            &index,
            with,
            &corpus.questions,
            &Fusion::default(),
        );

        println!(
            "\n\x1b[1m{}\x1b[0m — {} questions, lexical-only passes {}",
            corpus.name.to_uppercase(),
            corpus.questions.len(),
            lexical.passed()
        );
        for (label, fusion) in &plans {
            let card = run_fused(
                &corpus.ledger,
                &projection,
                &index,
                with,
                &corpus.questions,
                fusion,
            );
            println!(
                "  {label:<14} {:>2}/{} passed   {}",
                card.passed(),
                card.graded.len(),
                flips(&shipped, &card)
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
        .map(|(a, b)| {
            format!("{} {}->{}", a.question.id, a.verdict.label(), b.verdict.label())
        })
        .collect();
    if moved.is_empty() { "—".to_string() } else { moved.join("  ") }
}

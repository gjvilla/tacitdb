//! What meaning buys, measured — U-23's question put to a real model.
//!
//! `cargo run --release -p tacit-keeper --features real-embedder --example meaning`
//!
//! Runs both suites twice: once under the shipped hashing embedder, once
//! under a real embedding model, holding everything else still. Three things
//! come out: the score deltas, the per-question verdict flips, and the
//! similarity separation — D-0020 refused to let similarity confer confidence
//! because the hashing model's answerable and unanswerable distributions
//! overlapped (0.49–0.66 against 0.47–0.60), and that refusal is
//! model-relative, so the same two ranges are printed for the real model.
//!
//! First use downloads the model (~66MB) into fastembed's cache; every later
//! run is local.

use std::path::PathBuf;
use tacit_core::{
    Embedder, HashingEmbedder, Ledger, Projection, Query, TextIndex, VectorIndex, ViewSpec,
};
use tacit_keeper::corpus::ingest_corpus;
use tacit_keeper::embed::RealEmbedder;
use tacit_keeper::golden::{Expectation, GoldenQuestion, Scorecard, parse_golden, parse_golden_rows, run_with};
use tacit_keeper::pep::{ingest_peps, parse_pep};

struct Corpus {
    name: &'static str,
    ledger: Ledger,
    questions: Vec<GoldenQuestion>,
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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

    eprintln!("loading the model (downloads ~66MB on first use)…");
    let real = RealEmbedder::new()?;
    let hashing = HashingEmbedder::default();

    for corpus in &corpora {
        let projection = Projection::rebuild(&corpus.ledger);
        let index = TextIndex::rebuild(&corpus.ledger);

        let hashing_vectors = VectorIndex::rebuild(&corpus.ledger, &hashing);
        let base = run_with(
            &corpus.ledger,
            &projection,
            &index,
            Some((&hashing_vectors, &hashing as &dyn Embedder)),
            &corpus.questions,
        );
        let real_vectors = VectorIndex::rebuild(&corpus.ledger, &real);
        let card = run_with(
            &corpus.ledger,
            &projection,
            &index,
            Some((&real_vectors, &real as &dyn Embedder)),
            &corpus.questions,
        );

        println!(
            "\n\x1b[1m{}\x1b[0m — {} questions",
            corpus.name.to_uppercase(),
            corpus.questions.len()
        );
        println!(
            "  {:<28} {:>2}/{}   {:<28} {:>2}/{}",
            hashing.model_id(),
            base.passed(),
            base.graded.len(),
            real.model_id(),
            card.passed(),
            card.graded.len()
        );
        for (before, after) in base.graded.iter().zip(&card.graded) {
            if before.verdict != after.verdict {
                println!(
                    "    {} {:<15} -> {:<15} {}",
                    before.question.id,
                    before.verdict.label(),
                    after.verdict.label(),
                    truncate(&before.question.question, 44)
                );
            }
        }
        separation(corpus, &projection, &index, &real_vectors, &real, "real");
        separation(corpus, &projection, &index, &hashing_vectors, &hashing, "hashing");
        known_shortfalls(&card);
    }
    Ok(())
}

/// Top vector similarity per question, split by what the question deserves.
/// The two ranges D-0020's refusal rests on, recomputed for this model.
fn separation(
    corpus: &Corpus,
    projection: &Projection,
    index: &TextIndex,
    vectors: &VectorIndex,
    embedder: &dyn Embedder,
    label: &str,
) {
    let retriever = index
        .retriever(&corpus.ledger, projection, ViewSpec::now())
        .with_vectors(vectors, embedder);
    let mut answerable: Vec<f64> = Vec::new();
    let mut unanswerable: Vec<f64> = Vec::new();
    for question in &corpus.questions {
        let (_, vector) = retriever.candidates(&Query::text(&question.question));
        let top = vector.first().map(|(_, s)| *s).unwrap_or(0.0);
        match &question.expect {
            Expectation::Answer(_) => answerable.push(top),
            Expectation::Abstain { .. } => unanswerable.push(top),
        }
    }
    let range = |v: &[f64]| {
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for s in v {
            lo = lo.min(*s);
            hi = hi.max(*s);
        }
        if v.is_empty() { (0.0, 0.0) } else { (lo, hi) }
    };
    let (alo, ahi) = range(&answerable);
    let (ulo, uhi) = range(&unanswerable);
    println!(
        "  top similarity, {label:<7}  answerable {alo:.2}–{ahi:.2}   unanswerable {ulo:.2}–{uhi:.2}   {}",
        if ulo > 0.0 && alo > uhi { "SEPARATED" } else { "overlapping" }
    );
}

fn known_shortfalls(card: &Scorecard) {
    let still: Vec<&str> = card
        .graded
        .iter()
        .filter(|g| !g.verdict.is_pass())
        .map(|g| g.question.id.as_str())
        .collect();
    if !still.is_empty() {
        println!("  still failing under the real model: {}", still.join(" "));
    }
}

fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    format!("{}…", text.chars().take(width - 1).collect::<String>())
}

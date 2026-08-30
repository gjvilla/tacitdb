//! The quantities the confidence rule reads, for every question of both
//! suites — the instrument U-38 needs before any move on that rule is
//! believed.
//!
//! `cargo run --release -p tacit-keeper --example calibration`
//!
//! Per question: coverage of the first item, reach (how much of the question
//! the corpus can speak to at all), their ratio, and the margin — the top
//! lexical score over the best score of a record about a *different* subject,
//! because a record's own title and body both rank and a margin over yourself
//! is not a margin.
//!
//! This table is what refused U-38's proposed rule (D-0042): it put a bluff
//! at margin 1.02 beside an honest answer at 1.01, showed the rule's
//! motivating question drifted out of its own precondition, and held two
//! questions with identical readings needing opposite outcomes. It stays
//! because the refusal is corpus-relative: any future move on the confidence
//! rule starts by reading this off both corpora again.

use std::collections::BTreeSet;
use std::path::PathBuf;
use tacit_core::{
    Content, Embedder, EntityId, HashingEmbedder, Ledger, Projection, Query, RecordId, TextIndex,
    VectorIndex, ViewSpec,
};
use tacit_keeper::corpus::ingest_corpus;
use tacit_keeper::golden::{Expectation, GoldenQuestion, parse_golden, parse_golden_rows, run_with};
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

    for corpus in &corpora {
        let projection = Projection::rebuild(&corpus.ledger);
        let index = TextIndex::rebuild(&corpus.ledger);
        let embedder = HashingEmbedder::default();
        let vectors = VectorIndex::rebuild(&corpus.ledger, &embedder);
        let retriever = index
            .retriever(&corpus.ledger, &projection, ViewSpec::now())
            .with_vectors(&vectors, &embedder as &dyn Embedder);
        let card = run_with(
            &corpus.ledger,
            &projection,
            &index,
            Some((&vectors, &embedder as &dyn Embedder)),
            &corpus.questions,
        );

        println!("\n\x1b[1m{}\x1b[0m", corpus.name.to_uppercase());
        println!(
            "  {:<5} {:<7} {:<15} {:>5} {:>5} {:>6} {:>7}  top",
            "id", "expect", "verdict", "cov", "reach", "ratio", "margin"
        );
        for graded in &card.graded {
            let question = &graded.question;
            let found = retriever.retrieve(&Query::text(&question.question));
            let (lexical, _) = retriever.candidates(&Query::text(&question.question));
            let margin = margin(&corpus.ledger, &lexical);
            let ratio = if graded.known > 0.0 { graded.coverage / graded.known } else { 0.0 };
            println!(
                "  {:<5} {:<7} {:<15} {:>5.2} {:>5.2} {:>6.2} {:>7}  {}",
                question.id,
                match &question.expect {
                    Expectation::Answer(_) => "answer",
                    Expectation::Abstain { .. } => "abstain",
                },
                graded.verdict.label(),
                graded.coverage,
                graded.known,
                ratio,
                margin.map(|m| format!("{m:.2}")).unwrap_or_else(|| "solo".into()),
                found
                    .items
                    .first()
                    .map(|i| anchor(&corpus.ledger, i.record.id()).unwrap_or_default())
                    .unwrap_or_default(),
            );
        }
    }
    Ok(())
}

/// Top lexical score over the best score of a record about different
/// subjects. `None` when no record about anything else scored at all.
fn margin(ledger: &Ledger, lexical: &[(RecordId, f64)]) -> Option<f64> {
    let (top_id, top_score) = lexical.first()?;
    let top_about = subjects(ledger, *top_id);
    let rival = lexical
        .iter()
        .find(|(id, _)| subjects(ledger, *id) != top_about)
        .map(|(_, score)| *score)?;
    Some(top_score / rival)
}

fn subjects(ledger: &Ledger, id: RecordId) -> BTreeSet<EntityId> {
    ledger
        .record(id)
        .map(|record| match record.content() {
            Content::Claim(claim) => claim.entity_refs().into_iter().collect(),
            Content::Gap(gap) => gap.territory.iter().copied().collect(),
            _ => BTreeSet::new(),
        })
        .unwrap_or_default()
}

fn anchor(ledger: &Ledger, id: RecordId) -> Option<String> {
    subjects(ledger, id)
        .iter()
        .filter_map(|e| ledger.entity(*e))
        .map(|e| e.label().to_string())
        .next()
}

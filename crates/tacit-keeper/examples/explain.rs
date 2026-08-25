//! Why retrieval ranked what it ranked.
//!
//! `cargo run -p tacit-keeper --example explain [G-09 G-13 ...]`
//!
//! The golden suite grades outcomes; this is the instrument for the step before
//! them. "The suite is red" is not a diagnosis, and the four things it can mean
//! need four different repairs: no ranker found the answer, one found it and
//! fusion lost it, everything found it but the question was scored as weakly
//! covered, or the question is phrased in words the corpus has never used.
//!
//! It exists because every one of those was guessed at least once during U-23,
//! and the guesses were wrong more often than right. Tuning the fusion constant
//! across six values changed nothing at all; the actual fault was that a query
//! asking about *keys* could not reach a record saying *key*.

use std::collections::BTreeMap;
use std::path::PathBuf;
use tacit_core::{
    Content, Embedder, HashingEmbedder, Ledger, Projection, Query, Record, RecordId, TextIndex,
    VectorIndex, ViewSpec, indexable_text, tokenize,
};
use tacit_keeper::corpus::ingest_corpus;
use tacit_keeper::golden::{Expectation, parse_golden};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let only: Vec<String> = std::env::args().skip(1).collect();

    let mut ledger = Ledger::new();
    ingest_corpus(&mut ledger, &repo)?;
    let projection = Projection::rebuild(&ledger);
    let index = TextIndex::rebuild(&ledger);
    let embedder = HashingEmbedder::default();
    let vectors = VectorIndex::rebuild(&ledger, &embedder);
    let questions = parse_golden(&std::fs::read_to_string(repo.join("docs/GOLDEN.md"))?)?;

    // Document frequency over what the default view admits, so "how rare is
    // this word here" can be shown beside the ranking it produced.
    let mut df: BTreeMap<String, usize> = BTreeMap::new();
    let mut docs = 0usize;
    for record in ledger.records() {
        let Some(text) = indexable_text(record) else { continue };
        docs += 1;
        for term in tokenize(&text).into_iter().collect::<std::collections::BTreeSet<_>>() {
            *df.entry(term).or_default() += 1;
        }
    }
    println!("corpus: {docs} indexed records");
    for (id, where_, run) in tacit_keeper::quoted_questions(&questions, &ledger) {
        println!("  QUOTED: {id} appears in {where_} as a run of {run} words");
    }

    let retriever = index
        .retriever(&ledger, &projection, ViewSpec::now())
        .with_vectors(&vectors, &embedder as &dyn Embedder);

    for question in &questions {
        if !only.is_empty() && !only.contains(&question.id) {
            continue;
        }
        let wanted = match &question.expect {
            Expectation::Answer(id) => id.clone(),
            Expectation::Abstain { gap } => gap.clone().unwrap_or_else(|| "—".into()),
        };
        let query = Query::text(&question.question);
        let found = retriever.retrieve(&query);
        let (lexical, vector) = retriever.candidates(&query);

        println!(
            "\n{}  expect {wanted}  {:?}  coverage {:.2}  reach {:.2}",
            question.id, found.outcome, found.coverage, found.known
        );
        println!("  q: {}", question.question);
        if !found.read_as.is_empty() {
            println!(
                "  read as: {}",
                found
                    .read_as
                    .iter()
                    .map(|(asked, near)| format!("{asked}->{near}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
        println!(
            "  fused:   {}",
            found
                .items
                .iter()
                .take(4)
                .map(|i| format!("{}({:.1})", anchor(&ledger, i.record), i.relevance))
                .collect::<Vec<_>>()
                .join(" ")
        );
        println!("  lexical: {}", rank_line(&ledger, &lexical, &wanted));
        println!("  vector:  {}", rank_line(&ledger, &vector, &wanted));
        // A term the corpus has never seen is the difference between "this
        // record answered badly" and "nothing here could have answered".
        println!(
            "  terms:   {}",
            tokenize(&question.question)
                .iter()
                .map(|t| match df.get(t) {
                    Some(n) => format!("{t}:{n}"),
                    None => format!("{t}:none"),
                })
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
    Ok(())
}

/// The top of one ranker's list, and where the expected record sits in it.
fn rank_line(ledger: &Ledger, ranking: &[(RecordId, f64)], wanted: &str) -> String {
    let at = ranking
        .iter()
        .position(|(id, _)| ledger.record(*id).is_some_and(|r| anchor(ledger, r) == wanted));
    let top: Vec<String> = ranking
        .iter()
        .take(3)
        .filter_map(|(id, score)| {
            ledger.record(*id).map(|r| format!("{}({score:.2})", anchor(ledger, r)))
        })
        .collect();
    format!(
        "{:<44} expected@{}",
        top.join(" "),
        at.map(|p| p.to_string()).unwrap_or_else(|| "absent".into())
    )
}

/// The corpus id a record belongs to, so a ranking reads in the document's own
/// vocabulary rather than in record ids.
fn anchor(ledger: &Ledger, record: &Record) -> String {
    let refs = match record.content() {
        Content::Claim(claim) => claim.entity_refs(),
        Content::Gap(gap) => gap.territory.clone(),
        _ => Vec::new(),
    };
    for entity in refs {
        if let Some(e) = ledger.entity(entity)
            && (e.kind() == tacit_keeper::DECISION_KIND || e.kind() == tacit_keeper::UNKNOWN_KIND)
        {
            return e.label().to_string();
        }
    }
    record.id().to_string()
}

//! Grade the engine against the golden suite.
//!
//! `cargo run -p tacit-keeper --example golden`
//!
//! Exits non-zero on a regression — a failure nothing predicted. Known
//! shortfalls are reported and counted, not treated as passes and not treated
//! as breakage.

use std::path::PathBuf;
use std::process::ExitCode;
use tacit_core::{HashingEmbedder, Ledger, Projection, TextIndex, VectorIndex};
use tacit_keeper::corpus::ingest_corpus;
use tacit_keeper::golden::{Scorecard, Verdict, parse_golden, run_with};

fn main() -> ExitCode {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut ledger = Ledger::new();
    if let Err(error) = ingest_corpus(&mut ledger, &repo) {
        eprintln!("could not load the corpus: {error}");
        return ExitCode::FAILURE;
    }
    let text = match std::fs::read_to_string(repo.join("docs/GOLDEN.md")) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("could not read docs/GOLDEN.md: {error}");
            return ExitCode::FAILURE;
        }
    };
    let questions = match parse_golden(&text) {
        Ok(questions) => questions,
        Err(error) => {
            eprintln!("could not parse the golden suite: {error}");
            return ExitCode::FAILURE;
        }
    };

    let projection = Projection::rebuild(&ledger);
    let index = TextIndex::rebuild(&ledger);
    let embedder = HashingEmbedder::default();
    let vectors = VectorIndex::rebuild(&ledger, &embedder);

    // Both plans, so the second ranker's effect is measured rather than
    // asserted.
    let lexical_only = run_with(&ledger, &projection, &index, None, &questions);
    let card = run_with(
        &ledger,
        &projection,
        &index,
        Some((&vectors, &embedder as &dyn tacit_core::Embedder)),
        &questions,
    );

    println!("\n\x1b[1mGOLDEN SUITE\x1b[0m");
    println!("{}", "─".repeat(64));
    for graded in &card.graded {
        let mark = if graded.verdict.is_pass() {
            "pass"
        } else if graded.is_known_shortfall() {
            "known"
        } else {
            "FAIL"
        };
        println!(
            "  {mark:<5} {:<5} {:<15} {}",
            graded.question.id,
            graded.verdict.label(),
            truncate(&graded.question.question, 46)
        );
        if !graded.verdict.is_pass() {
            println!(
                "              expected {:?}, got tags {} top {:?} gaps {:?}",
                graded.question.expect,
                graded.tags.join("+"),
                graded.top,
                graded.cited_gaps
            );
            println!("              to address: {}", graded.verdict.owner());
        }
    }

    let total = card.graded.len();
    println!("\n\x1b[1mSCORE\x1b[0m");
    println!("{}", "─".repeat(64));
    println!("  {}/{} passed", card.passed(), total);
    println!(
        "  {} of those were earned by declining to answer",
        card.abstentions_rewarded()
    );
    println!("  {} known shortfalls (tracked against a registered unknown)", card.known_shortfalls().len());
    println!("  {} regressions", card.regressions().len());

    for graded in card.recovered() {
        println!(
            "  RECOVERED  {} now passes; {} can be reconsidered",
            graded.question.id,
            graded.question.pending.clone().unwrap_or_default()
        );
    }
    let ungoverned = card.ungoverned();
    if !ungoverned.is_empty() {
        println!("  {} questions lack an owner or a review trigger", ungoverned.len());
    }

    let mut by_kind: Vec<(Verdict, usize)> = Vec::new();
    for graded in &card.graded {
        match by_kind.iter_mut().find(|(v, _)| *v == graded.verdict) {
            Some((_, count)) => *count += 1,
            None => by_kind.push((graded.verdict, 1)),
        }
    }
    println!("\n  by condition:");
    for (verdict, count) in by_kind {
        println!("    {count:>2}  {:<15} {}", verdict.label(), verdict.owner());
    }

    println!(
        "\n  An accuracy score would read {}/{}. It would also reward a system that\n  \
         answered every one of the {} unanswerable questions confidently — which is\n  \
         why the abstentions are counted separately and the failures are named.",
        card.passed(),
        total,
        card.graded
            .iter()
            .filter(|g| matches!(g.question.expect, tacit_keeper::Expectation::Abstain { .. }))
            .count()
    );

    compare(&lexical_only, &card, &vectors, &embedder);

    // An agreed answer goes stale the moment the thing it was agreed about
    // changes, and every question carries a trigger saying when to re-read it.
    // Nothing was checking them, which is how `abstain U-5` survived a day past
    // U-5 being resolved — unsatisfiable, and passing.
    let register = std::fs::read_to_string(repo.join("docs/REGISTER.md")).unwrap_or_default();
    let unknowns = tacit_keeper::parse_register(&register).unwrap_or_default();
    let stale = tacit_keeper::stale_triggers(&questions, &unknowns);
    if !stale.is_empty() {
        println!("\n\x1b[1mTRIGGERS THAT HAVE FIRED\x1b[0m");
        println!("{}", "─".repeat(64));
        for (id, why) in &stale {
            println!("  {id}  {why}");
        }
        println!();
        println!("  Re-read each question against the record as it now stands, then give it");
        println!("  a trigger that has not already fired. A suite nobody re-reads is a");
        println!("  record of what was once true.");
    }

    // A corpus that describes its own retrieval failures will quote the
    // questions that fail, and then rank for them. Caught here because the
    // manual check U-27 asked for demonstrably does not happen: two of these
    // were written in a single commit, and one moved a question's reach from
    // 0.52 to 1.00.
    let quoted = tacit_keeper::quoted_questions(&questions, &ledger);
    if !quoted.is_empty() {
        println!("\n\x1b[1mQUESTIONS THE CORPUS QUOTES BACK\x1b[0m");
        println!("{}", "─".repeat(64));
        for (id, where_, run) in &quoted {
            println!("  {id}  quoted in {where_}, {run} words in a row");
        }
        println!();
        println!("  Name a golden question by its id in the corpus, never by its wording.");
        println!("  A record that repeats the question ranks for it, and then the record");
        println!("  explaining a failure outranks the record that would answer it (U-27).");
    }

    if card.regressions().is_empty() && stale.is_empty() && quoted.is_empty() {
        println!();
        ExitCode::SUCCESS
    } else {
        if !card.regressions().is_empty() {
            println!("\n  {} regression(s) — the suite is red.", card.regressions().len());
        }
        if !stale.is_empty() {
            println!("  {} question(s) resting on a trigger that has fired.", stale.len());
        }
        if !quoted.is_empty() {
            println!("  {} question(s) quoted back by the corpus.", quoted.len());
        }
        println!();
        ExitCode::FAILURE
    }
}

/// What the second ranker changed, question by question.
fn compare(
    lexical: &Scorecard,
    hybrid: &Scorecard,
    vectors: &VectorIndex,
    embedder: &HashingEmbedder,
) {
    use tacit_core::Embedder as _;
    println!("\n\x1b[1mWHAT VECTOR CANDIDATES CHANGED\x1b[0m");
    println!("{}", "─".repeat(64));
    println!(
        "  {} vectors, model {} ({} dimensions)",
        vectors.len(),
        embedder.model_id(),
        embedder.dimensions()
    );
    println!(
        "  lexical only: {}/{} passed   with vectors: {}/{} passed",
        lexical.passed(),
        lexical.graded.len(),
        hybrid.passed(),
        hybrid.graded.len()
    );
    let mut moved = 0;
    for (before, after) in lexical.graded.iter().zip(&hybrid.graded) {
        if before.verdict != after.verdict {
            moved += 1;
            println!(
                "    {} {:<15} -> {:<15} {}",
                before.question.id,
                before.verdict.label(),
                after.verdict.label(),
                truncate(&before.question.question, 40)
            );
        }
    }
    if moved == 0 {
        println!("    nothing moved.");
    }
}

fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    format!("{}…", text.chars().take(width - 1).collect::<String>())
}

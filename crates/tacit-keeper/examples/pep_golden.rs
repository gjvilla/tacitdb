//! Grade retrieval against the proposals corpus — real language this project
//! did not write.
//!
//! `cargo run -p tacit-keeper --example pep_golden -- [dir]`
//!
//! The directory defaults to `target/proposals`; `scripts/fetch-proposals.sh`
//! fills it from the pinned upstream commit. The questions live in
//! `docs/PEP-GOLDEN.md` and the corpus lives outside the repository (U-11), so
//! the runner's first job is to check the two still refer to each other: a
//! suite agreed against one slice and run over another measures nothing, and
//! says so here rather than producing a number anyway.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::ExitCode;
use tacit_core::{
    Content, HashingEmbedder, Ledger, Projection, TextIndex, VectorIndex, indexable_text, tokenize,
};
use tacit_keeper::golden::{parse_baseline_rows, parse_golden_rows, run_with};
use tacit_keeper::pep::{Pep, ingest_peps, parse_pep};

fn main() -> ExitCode {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.join("target/proposals"));
    if !dir.is_dir() {
        eprintln!("no corpus at {}", dir.display());
        eprintln!("fetch the pinned slice first: scripts/fetch-proposals.sh");
        return ExitCode::FAILURE;
    }

    let doc = match std::fs::read_to_string(repo.join("docs/PEP-GOLDEN.md")) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("could not read docs/PEP-GOLDEN.md: {error}");
            return ExitCode::FAILURE;
        }
    };
    let questions = match parse_golden_rows(&doc, "P-") {
        Ok(questions) => questions,
        Err(error) => {
            eprintln!("could not parse the suite: {error}");
            return ExitCode::FAILURE;
        }
    };

    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "rst" || e == "txt"))
        .collect();
    files.sort();
    let mut peps: Vec<Pep> = Vec::new();
    let mut refused = 0usize;
    for path in &files {
        let Ok(text) = std::fs::read_to_string(path) else { continue };
        match parse_pep(&text) {
            Ok(pep) => peps.push(pep),
            Err(error) => {
                eprintln!("REFUSED {}: {error}", path.display());
                refused += 1;
            }
        }
    }
    peps.sort_by_key(|p| p.number);

    // The pin check. The document records which labels the questions were
    // agreed against; a corpus holding anything else is a different corpus.
    let agreed = pinned_labels(&doc);
    let supplied: BTreeSet<String> = peps.iter().map(Pep::label).collect();
    let missing: Vec<&String> = agreed.difference(&supplied).collect();
    let extra: Vec<&String> = supplied.difference(&agreed).collect();
    if agreed.is_empty() {
        eprintln!("docs/PEP-GOLDEN.md lists no pinned slice — nothing to grade against");
        return ExitCode::FAILURE;
    }
    if !missing.is_empty() || !extra.is_empty() || refused > 0 {
        eprintln!("the corpus is not the slice the suite was agreed against:");
        if !missing.is_empty() {
            eprintln!("  missing {missing:?}");
        }
        if !extra.is_empty() {
            eprintln!("  extra {extra:?}");
        }
        if refused > 0 {
            eprintln!("  {refused} documents refused by the parser");
        }
        return ExitCode::FAILURE;
    }

    let mut ledger = Ledger::new();
    let report = match ingest_peps(&mut ledger, &peps) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("ingest failed: {error}");
            return ExitCode::FAILURE;
        }
    };

    let projection = Projection::rebuild(&ledger);
    let index = TextIndex::rebuild(&ledger);
    let embedder = HashingEmbedder::default();
    let vectors = VectorIndex::rebuild(&ledger, &embedder);

    println!("\n\x1b[1mPROPOSALS SUITE\x1b[0m");
    println!("{}", "─".repeat(72));
    println!(
        "  {} proposals, {} records, {} indexed documents",
        report.proposals,
        report.records,
        index.documents()
    );
    length_panel(&ledger);

    // Both plans, as the self-corpus suite runs them: the second ranker's
    // effect is measured, not asserted.
    let lexical_only = run_with(&ledger, &projection, &index, None, &questions);
    let card = run_with(
        &ledger,
        &projection,
        &index,
        Some((&vectors, &embedder as &dyn tacit_core::Embedder)),
        &questions,
    );

    println!("\n  {:<5} {:<5} {:<15} {:>5} {:>5}  question", "", "id", "verdict", "cov", "reach");
    for graded in &card.graded {
        let mark = if graded.verdict.is_pass() {
            "pass"
        } else if graded.is_known_shortfall() {
            "known"
        } else {
            "FAIL"
        };
        println!(
            "  {mark:<5} {:<5} {:<15} {:>5.2} {:>5.2}  {}",
            graded.question.id,
            graded.verdict.label(),
            graded.coverage,
            graded.known,
            truncate(&graded.question.question, 48)
        );
        if !graded.verdict.is_pass() {
            println!(
                "               expected {:?}, got tags {} top {:?}",
                graded.question.expect,
                graded.tags.join("+"),
                graded.top
            );
            println!("               to address: {}", graded.verdict.owner());
        }
    }

    let total = card.graded.len();
    println!("\n\x1b[1mSCORE\x1b[0m");
    println!("{}", "─".repeat(72));
    println!("  {}/{} passed", card.passed(), total);
    println!("  {} of those were earned by declining to answer", card.abstentions_rewarded());
    println!("  {} known shortfalls (tracked against a registered unknown)", card.known_shortfalls().len());
    println!("  {} regressions", card.regressions().len());
    for graded in card.recovered() {
        println!(
            "  RECOVERED  {} now passes; {} can be reconsidered",
            graded.question.id,
            graded.question.pending.clone().unwrap_or_default()
        );
    }
    println!(
        "  lexical only: {}/{} passed   with vectors: {}/{} passed",
        lexical_only.passed(),
        total,
        card.passed(),
        total
    );
    for (before, after) in lexical_only.graded.iter().zip(&card.graded) {
        if before.verdict != after.verdict {
            println!(
                "    {} {:<15} -> {:<15} under vector candidates",
                before.question.id,
                before.verdict.label(),
                after.verdict.label()
            );
        }
    }

    // The same two vocabulary instruments the self-corpus suite runs. The
    // corpus is pinned, so drift can only arrive by repinning the slice — at
    // which point every question is due a re-read and this is the reminder.
    let current = tacit_keeper::absent_vocabulary(&questions, &ledger);
    let recorded = parse_baseline_rows(&doc, "P-");
    let drifted = tacit_keeper::vocabulary_drift(&recorded, &current);
    let unrecorded = tacit_keeper::missing_baseline(&recorded, &current);
    for (id, words) in &drifted {
        println!("  DRIFT {id}: the corpus now contains {}", words.join(", "));
    }
    if !unrecorded.is_empty() && std::env::var("PEP_GOLDEN_BASELINE").is_ok() {
        println!("\n## Vocabulary baseline\n");
        println!("| id | words the corpus did not contain |");
        println!("|----|----------------------------------|");
        for (id, words) in &current {
            println!("| {id} | {} |", if words.is_empty() { "—".into() } else { words.join(" ") });
        }
    } else if !unrecorded.is_empty() {
        println!(
            "  {} question(s) lack a vocabulary baseline — PEP_GOLDEN_BASELINE=1 prints one",
            unrecorded.len()
        );
    }
    let quoted = tacit_keeper::quoted_questions(&questions, &ledger);
    for (id, where_, run) in &quoted {
        println!("  QUOTED {id}: {run} words in a row shared with {where_}");
    }

    println!();
    if card.regressions().is_empty() && drifted.is_empty() && quoted.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// The `PEP-XXXX` labels named in the document's `## Pinned slice` section.
fn pinned_labels(doc: &str) -> BTreeSet<String> {
    let mut labels = BTreeSet::new();
    let mut in_section = false;
    for line in doc.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix("## ") {
            in_section = heading.to_lowercase().contains("pinned slice");
            continue;
        }
        if !in_section {
            continue;
        }
        let mut rest = trimmed;
        while let Some(at) = rest.find("PEP-") {
            let tail = &rest[at + 4..];
            let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();
            if digits.len() == 4 {
                labels.insert(format!("PEP-{digits}"));
            }
            rest = &tail[digits.len()..];
        }
    }
    labels
}

/// What U-39 is about, measured on this corpus: how document lengths fall when
/// nobody chose them to be kind.
fn length_panel(ledger: &Ledger) {
    let mut titles: Vec<usize> = Vec::new();
    let mut bodies: Vec<usize> = Vec::new();
    let mut edges: Vec<usize> = Vec::new();
    for record in ledger.records() {
        let Some(text) = indexable_text(record) else { continue };
        let tokens = tokenize(&text).len();
        match record.content() {
            Content::Claim(tacit_core::ClaimContent::Text { .. }) => bodies.push(tokens),
            Content::Claim(tacit_core::ClaimContent::Attribute { .. }) => titles.push(tokens),
            _ => edges.push(tokens),
        }
    }
    let stats = |v: &[usize]| -> (usize, usize) {
        if v.is_empty() {
            return (0, 0);
        }
        (v.iter().sum::<usize>() / v.len(), *v.iter().max().unwrap_or(&0))
    };
    let (title_avg, title_max) = stats(&titles);
    let (body_avg, body_max) = stats(&bodies);
    let (edge_avg, _) = stats(&edges);
    println!(
        "  lengths (tokens): {} titles avg {title_avg} max {title_max}; {} bodies avg {body_avg} max {body_max}; {} edges avg {edge_avg}",
        titles.len(),
        bodies.len(),
        edges.len()
    );
}

fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    format!("{}…", text.chars().take(width - 1).collect::<String>())
}

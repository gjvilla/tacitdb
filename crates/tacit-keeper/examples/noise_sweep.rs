//! What assembly noise costs to remove, measured on both suites at once.
//!
//! `cargo run --release -p tacit-keeper --example noise_sweep`
//!
//! Two kinds of item a reader called noise on first contact (D-0058): a
//! record reached by similarity alone that covers none of the question, and
//! a record's title claim listed beside its body. Removing either is a
//! change to the assembled list the suites grade, so — U-41's rule, kept —
//! it is believed only after both corpora have been graded with it on and
//! off. This instrument prints the pass counts, every question that moved,
//! and the amount of noise each setting actually removes, so the decision is
//! made on numbers rather than on the reader's first impression. The first
//! row removes nothing and is the baseline the others are compared against;
//! the shipped default since D-0058 is the last row.

use std::path::PathBuf;
use tacit_core::{
    ClaimContent, Content, Embedder, HashingEmbedder, Ledger, Projection, Query, TextIndex,
    TitleFold, VectorIndex, Via, ViewSpec,
};
use tacit_keeper::corpus::ingest_corpus;
use tacit_keeper::golden::{GoldenQuestion, Scorecard, parse_golden, parse_golden_rows, run_configured};
use tacit_keeper::pep::{ingest_peps, parse_pep};

struct Corpus {
    name: &'static str,
    ledger: Ledger,
    questions: Vec<GoldenQuestion>,
}

struct Setting {
    label: &'static str,
    drop_uncovered: bool,
    titles: TitleFold,
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

    let settings = [
        Setting { label: "nothing removed", drop_uncovered: false, titles: TitleFold::Keep },
        Setting { label: "drop uncovered", drop_uncovered: true, titles: TitleFold::Keep },
        Setting { label: "fold titles behind", drop_uncovered: false, titles: TitleFold::FoldBehind },
        Setting { label: "prefer bodies", drop_uncovered: false, titles: TitleFold::PreferBody },
        Setting { label: "drop + fold behind", drop_uncovered: true, titles: TitleFold::FoldBehind },
        Setting { label: "drop + prefer bodies", drop_uncovered: true, titles: TitleFold::PreferBody },
    ];

    for corpus in &corpora {
        let projection = Projection::rebuild(&corpus.ledger);
        let index = TextIndex::rebuild(&corpus.ledger);
        let embedder = HashingEmbedder::default();
        let vectors = VectorIndex::rebuild(&corpus.ledger, &embedder);
        let with = Some((&vectors, &embedder as &dyn Embedder));
        let retriever = index
            .retriever(&corpus.ledger, &projection, ViewSpec::now())
            .with_vectors(&vectors, &embedder as &dyn Embedder);

        println!(
            "\n\x1b[1m{}\x1b[0m — {} questions",
            corpus.name.to_uppercase(),
            corpus.questions.len()
        );
        println!(
            "  {:<22} {:>6}   {:>9} {:>9} {:>9}   moved",
            "setting", "passed", "items", "uncovered", "dup-title"
        );
        let mut shipped: Option<Scorecard> = None;
        for setting in &settings {
            let configure = |query: &mut Query| {
                query.drop_uncovered = setting.drop_uncovered;
                query.titles = setting.titles;
            };
            let card = run_configured(
                &corpus.ledger,
                &projection,
                &index,
                with,
                &corpus.questions,
                &configure,
            );
            // Noise, counted directly over the assembled lists the suite
            // graded: items that share no word with the question and were
            // reached by similarity alone, and title claims listed beside an
            // item about the same subject.
            let (mut items, mut uncovered, mut dup_titles) = (0usize, 0usize, 0usize);
            for question in &corpus.questions {
                let mut query = Query::text(&question.question);
                configure(&mut query);
                let found = retriever.retrieve(&query);
                items += found.items.len();
                uncovered += found
                    .items
                    .iter()
                    .filter(|i| matches!(i.via, Via::Vector) && i.coverage <= 0.0)
                    .count();
                let subjects: Vec<Vec<_>> = found
                    .items
                    .iter()
                    .map(|i| match i.record.content() {
                        Content::Claim(c) => c.entity_refs(),
                        _ => Vec::new(),
                    })
                    .collect();
                for (n, item) in found.items.iter().enumerate() {
                    let is_title = matches!(
                        item.record.content(),
                        Content::Claim(ClaimContent::Attribute { name, .. }) if name == "title"
                    );
                    if is_title
                        && subjects
                            .iter()
                            .enumerate()
                            .any(|(m, refs)| m != n && refs.iter().any(|r| subjects[n].contains(r)))
                    {
                        dup_titles += 1;
                    }
                }
            }
            let moved = match &shipped {
                Some(base) => flips(base, &card),
                None => "— (baseline)".to_string(),
            };
            println!(
                "  {:<22} {:>2}/{:<3}   {:>9} {:>9} {:>9}   {moved}",
                setting.label,
                card.passed(),
                card.graded.len(),
                items,
                uncovered,
                dup_titles
            );
            if shipped.is_none() {
                shipped = Some(card);
            }
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

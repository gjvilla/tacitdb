//! The golden suite: representative questions, agreed answers, and a verdict
//! per question classified by which room the failure came from.
//!
//! Two things distinguish this from an accuracy score.
//!
//! **Abstention is a pass.** A question the record does not settle should come
//! back as an abstention, and a system that answers it confidently fails here.
//!
//! **Failures are classified, not counted.** "The record holds this and
//! retrieval missed it" and "the record does not hold this and retrieval
//! answered anyway" are different conditions with different owners. Collapsing
//! them into one number tells you nothing about what to fix.

use crate::parse::ParseError;
use std::collections::BTreeSet;
use tacit_core::{
    Content, Embedder, Ledger, Outcome, Projection, Query, Retrieved, TextIndex, VectorIndex,
    ViewSpec,
};

/// How far down the results an expected answer may appear and still count as
/// found. Beyond this it is a ranking failure, not a retrieval success.
const ANSWER_RANK_LIMIT: usize = 3;

#[derive(Debug, Clone, PartialEq)]
pub enum Expectation {
    /// The record settles this, and this record is the one that does.
    Answer(String),
    /// The record does not settle this. `gap` names the registered unknown
    /// that should be cited, when one covers the territory.
    Abstain { gap: Option<String> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct GoldenQuestion {
    pub id: String,
    pub question: String,
    pub expect: Expectation,
    pub owner: String,
    pub review_trigger: String,
    /// A registered unknown this question is known to fall short against
    /// today. Tracked as a shortfall rather than scored as a pass or failed as
    /// a regression.
    pub pending: Option<String>,
}

/// What happened, in the vocabulary of the four rooms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The record settled it and retrieval found it.
    Answered,
    /// The record did not settle it and retrieval said so. A pass, and the one
    /// an accuracy score would punish.
    Abstained,
    /// Abstained *and* cited the registered question covering the territory.
    AbstainedWithGap,
    /// The record holds the answer and retrieval did not surface it at all.
    Missed,
    /// Retrieval surfaced the right record but declined to call it a match.
    /// A calibration failure, not a recall one — and worth separating, because
    /// a consumer following the instructions would abstain on a question the
    /// record actually settles.
    Underconfident,
    /// The record does not hold the answer and retrieval answered anyway. The
    /// most costly failure here, and the reason abstention is scored at all.
    Bluffed,
    /// Answered confidently, but with the wrong record. A ranking failure.
    WrongAnchor,
    /// Abstained, but did not surface the registered question that covers the
    /// territory. Gap detection's to fix.
    GapNotCited,
}

impl Verdict {
    pub fn is_pass(self) -> bool {
        matches!(self, Verdict::Answered | Verdict::Abstained | Verdict::AbstainedWithGap)
    }

    /// Whose it is to address, per the failure taxonomy.
    pub fn owner(self) -> &'static str {
        match self {
            Verdict::Answered | Verdict::Abstained | Verdict::AbstainedWithGap => "nobody — a pass",
            Verdict::Missed => "retrieval: recall",
            Verdict::Underconfident => "retrieval: calibration",
            Verdict::WrongAnchor => "retrieval: ranking",
            Verdict::Bluffed => "retrieval: abstention",
            Verdict::GapNotCited => "retrieval: gap detection",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Verdict::Answered => "answered",
            Verdict::Abstained => "abstained",
            Verdict::AbstainedWithGap => "abstained+cited",
            Verdict::Missed => "MISSED",
            Verdict::Underconfident => "UNDERCONFIDENT",
            Verdict::Bluffed => "BLUFFED",
            Verdict::WrongAnchor => "WRONG ANCHOR",
            Verdict::GapNotCited => "GAP NOT CITED",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Graded {
    pub question: GoldenQuestion,
    pub verdict: Verdict,
    /// What actually came back, for the report.
    pub tags: Vec<String>,
    pub top: Vec<String>,
    pub cited_gaps: Vec<String>,
}

impl Graded {
    /// A shortfall is a known-weak question failing as expected: not a pass,
    /// not a regression, a tracked number.
    pub fn is_known_shortfall(&self) -> bool {
        self.question.pending.is_some() && !self.verdict.is_pass()
    }

    /// A failure that nothing predicted — the ones that should stop a build.
    pub fn is_regression(&self) -> bool {
        !self.verdict.is_pass() && self.question.pending.is_none()
    }

    /// A question marked known-weak that has started passing: the register
    /// entry can be closed, and the suite says so rather than staying quiet.
    pub fn is_recovered(&self) -> bool {
        self.question.pending.is_some() && self.verdict.is_pass()
    }
}

#[derive(Debug, Default)]
pub struct Scorecard {
    pub graded: Vec<Graded>,
}

impl Scorecard {
    pub fn passed(&self) -> usize {
        self.graded.iter().filter(|g| g.verdict.is_pass()).count()
    }

    /// Passes earned by declining to answer. Reported separately because this
    /// is the number a plain accuracy score destroys.
    pub fn abstentions_rewarded(&self) -> usize {
        self.graded
            .iter()
            .filter(|g| matches!(g.verdict, Verdict::Abstained | Verdict::AbstainedWithGap))
            .count()
    }

    pub fn regressions(&self) -> Vec<&Graded> {
        self.graded.iter().filter(|g| g.is_regression()).collect()
    }

    pub fn known_shortfalls(&self) -> Vec<&Graded> {
        self.graded.iter().filter(|g| g.is_known_shortfall()).collect()
    }

    pub fn recovered(&self) -> Vec<&Graded> {
        self.graded.iter().filter(|g| g.is_recovered()).collect()
    }

    /// Questions carrying no owner or no review trigger. Golden data is
    /// standard work and decays; an unowned expectation will eventually punish
    /// the engine for telling the new truth.
    pub fn ungoverned(&self) -> Vec<&Graded> {
        self.graded
            .iter()
            .filter(|g| g.question.owner.is_empty() || g.question.review_trigger.is_empty())
            .collect()
    }
}

/// Parse the `## Questions` table of `docs/GOLDEN.md`.
pub fn parse_golden(text: &str) -> Result<Vec<GoldenQuestion>, ParseError> {
    let mut questions = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("| G-") {
            continue;
        }
        let cells: Vec<&str> = trimmed
            .strip_prefix('|')
            .and_then(|l| l.strip_suffix('|'))
            .unwrap_or(trimmed)
            .split('|')
            .map(str::trim)
            .collect();
        if cells.len() != 5 {
            return Err(ParseError::BadRegisterRow {
                row: trimmed.chars().take(60).collect(),
                cells: cells.len(),
            });
        }
        let id = cells[0].to_string();
        if questions.iter().any(|q: &GoldenQuestion| q.id == id) {
            return Err(ParseError::DuplicateUnknown { id });
        }
        let (expect, pending) = parse_expectation(&id, cells[2])?;
        questions.push(GoldenQuestion {
            id,
            question: cells[1].to_string(),
            expect,
            owner: cells[3].to_string(),
            review_trigger: cells[4].to_string(),
            pending,
        });
    }
    Ok(questions)
}

fn parse_expectation(
    id: &str,
    text: &str,
) -> Result<(Expectation, Option<String>), ParseError> {
    let bad = || ParseError::BadRegisterRow { row: format!("{id}: {text:?}"), cells: 5 };
    let mut pending = None;
    let mut body = text.trim().to_string();
    if let Some(at) = body.find("pending ") {
        let tail = body[at + "pending ".len()..].trim().trim_end_matches(')').trim();
        pending = Some(tail.to_string());
        body = body[..at].trim().trim_end_matches('(').trim().to_string();
    }

    let mut words = body.split_whitespace();
    let expectation = match words.next().ok_or_else(bad)? {
        "answer" => Expectation::Answer(words.next().ok_or_else(bad)?.to_string()),
        "abstain" => Expectation::Abstain { gap: words.next().map(str::to_string) },
        _ => return Err(bad()),
    };
    Ok((expectation, pending))
}

/// Run every question against the record and grade it, lexical only.
pub fn run(
    ledger: &Ledger,
    projection: &Projection,
    index: &TextIndex,
    questions: &[GoldenQuestion],
) -> Scorecard {
    run_with(ledger, projection, index, None, questions)
}

/// Grade with vector candidates in the plan as well, so the suite can measure
/// what the second ranker actually changed rather than anyone asserting it.
/// Golden questions whose review trigger names a registered unknown that has
/// since been resolved, with what the trigger said.
///
/// The suite is a set of agreed answers, and an agreement goes stale the moment
/// the thing it was agreed about changes. Every question carries a trigger for
/// exactly that reason and nothing was checking them — so `abstain U-5` sat in
/// the suite for a day after U-5 was resolved, unsatisfiable (an answered gap
/// cannot be cited as registered) and *passing*, because the system failed to
/// answer a question it had since learned the answer to. Two failures cancelling
/// is the worst way for a test to be green.
pub fn stale_triggers(
    questions: &[GoldenQuestion],
    unknowns: &[crate::register::ParsedUnknown],
) -> Vec<(String, String)> {
    questions
        .iter()
        .filter_map(|question| {
            let named = crate::parse::mentioned_ids(&question.review_trigger, &question.id);
            let fired = named.iter().find(|id| {
                unknowns.iter().any(|u| u.id == **id && u.resolved.is_some())
            })?;
            Some((question.id.clone(), format!("{fired} is resolved: \"{}\"", question.review_trigger)))
        })
        .collect()
}

/// Golden questions whose wording has leaked into the corpus, with the record
/// that quotes them and how long the quoted run is.
///
/// A corpus that describes its own retrieval failures will quote the questions
/// that fail, and then rank for them — so the record explaining why a question
/// cannot be answered outranks the record that answers it. Not fixable in the
/// engine: it is a curation discipline, and a discipline nobody checks is a
/// wish (U-27).
pub fn quoted_questions(
    questions: &[GoldenQuestion],
    ledger: &Ledger,
) -> Vec<(String, String, usize)> {
    let mut found = Vec::new();
    for question in questions {
        let asked = tacit_core::tokenize(&question.question);
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for record in ledger.records() {
            let Some(text) = tacit_core::indexable_text(record) else { continue };
            let run = longest_shared_run(&asked, &tacit_core::tokenize(&text));
            // Every offender, not the worst one: two records each quoting the
            // same question is two records ranking for it.
            if run >= QUOTE_RUN && seen.insert(anchor_of(ledger, record)) {
                found.push((question.id.clone(), anchor_of(ledger, record), run));
            }
        }
    }
    found
}

/// How many of a question's words in a row a record may repeat before it starts
/// ranking for the question rather than for its subject. Five is a phrase.
const QUOTE_RUN: usize = 5;

fn longest_shared_run(asked: &[String], text: &[String]) -> usize {
    let mut best = 0;
    for start in 0..text.len() {
        for (from, _) in asked.iter().enumerate() {
            let mut run = 0;
            while from + run < asked.len()
                && start + run < text.len()
                && asked[from + run] == text[start + run]
            {
                run += 1;
            }
            best = best.max(run);
        }
    }
    best
}

pub fn run_with(
    ledger: &Ledger,
    projection: &Projection,
    index: &TextIndex,
    vectors: Option<(&VectorIndex, &dyn Embedder)>,
    questions: &[GoldenQuestion],
) -> Scorecard {
    let retriever = index.retriever(ledger, projection, ViewSpec::now());
    let retriever = match vectors {
        Some((index, embedder)) => retriever.with_vectors(index, embedder),
        None => retriever,
    };
    let graded = questions
        .iter()
        .map(|question| {
            let found = retriever.retrieve(&Query::text(&question.question));
            grade(ledger, question, &found)
        })
        .collect();
    Scorecard { graded }
}

fn grade(ledger: &Ledger, question: &GoldenQuestion, found: &Retrieved<'_>) -> Graded {
    let top: Vec<String> = found
        .items
        .iter()
        .take(ANSWER_RANK_LIMIT)
        .map(|item| anchor_of(ledger, item.record))
        .collect();
    let cited_gaps: Vec<String> =
        found.gaps.iter().map(|gap| anchor_of(ledger, gap)).collect();

    let verdict = match &question.expect {
        Expectation::Answer(expected) => {
            let found_it = top.iter().any(|a| a == expected);
            match (found_it, found.outcome == Outcome::Matches) {
                (true, true) => Verdict::Answered,
                (true, false) => Verdict::Underconfident,
                (false, true) => Verdict::WrongAnchor,
                (false, false) => Verdict::Missed,
            }
        }
        Expectation::Abstain { gap } => {
            if found.outcome == Outcome::Matches {
                Verdict::Bluffed
            } else {
                match gap {
                    Some(expected) if !cited_gaps.iter().any(|g| g == expected) => {
                        Verdict::GapNotCited
                    }
                    Some(_) => Verdict::AbstainedWithGap,
                    None => Verdict::Abstained,
                }
            }
        }
    };

    Graded { question: question.clone(), verdict, tags: found.tags().iter().map(|t| t.to_string()).collect(), top, cited_gaps }
}

/// The corpus label a record answers to (`D-0015`, `U-5`), for comparing
/// against an expectation written in those terms.
fn anchor_of(ledger: &Ledger, record: &tacit_core::Record) -> String {
    let entities = match record.content() {
        Content::Claim(claim) => claim.entity_refs(),
        Content::Gap(gap) => gap.territory.clone(),
        _ => Vec::new(),
    };
    for entity in entities {
        if let Some(e) = ledger.entity(entity)
            && matches!(e.kind(), "decision" | "unknown")
        {
            return e.label().to_string();
        }
    }
    record.id().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::ingest_corpus;
    use std::path::PathBuf;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// Grade the engine as it is actually configured — the retrieval plan the
    /// MCP host serves, vector candidates included. Grading a different plan
    /// than the one that ships would make the instrument measure nothing.
    struct Configured {
        ledger: Ledger,
        projection: Projection,
        index: TextIndex,
        vectors: tacit_core::VectorIndex,
        embedder: tacit_core::HashingEmbedder,
    }

    impl Configured {
        fn score(&self, questions: &[GoldenQuestion]) -> Scorecard {
            run_with(
                &self.ledger,
                &self.projection,
                &self.index,
                Some((&self.vectors, &self.embedder as &dyn Embedder)),
                questions,
            )
        }
    }

    fn corpus() -> Configured {
        let mut ledger = Ledger::new();
        ingest_corpus(&mut ledger, &repo_root()).expect("corpus loads");
        let projection = Projection::rebuild(&ledger);
        let index = TextIndex::rebuild(&ledger);
        let embedder = tacit_core::HashingEmbedder::default();
        let vectors = tacit_core::VectorIndex::rebuild(&ledger, &embedder);
        Configured { ledger, projection, index, vectors, embedder }
    }

    fn suite() -> Vec<GoldenQuestion> {
        let text = std::fs::read_to_string(repo_root().join("docs/GOLDEN.md")).unwrap();
        parse_golden(&text).expect("the suite parses")
    }

    #[test]
    fn expectations_parse_in_every_form() {
        let questions = suite();
        assert!(questions.len() >= 12, "the suite is loaded, not a fragment");
        assert!(questions.iter().any(|q| matches!(&q.expect, Expectation::Answer(_))));
        assert!(
            questions
                .iter()
                .any(|q| matches!(&q.expect, Expectation::Abstain { gap: Some(_) }))
        );
        assert!(
            questions.iter().any(|q| matches!(&q.expect, Expectation::Abstain { gap: None }))
        );
        assert!(questions.iter().any(|q| q.pending.is_some()));

        let pending = questions.iter().find(|q| q.pending.is_some()).unwrap();
        assert_eq!(pending.pending.as_deref(), Some("U-23"));
        // The expectation itself survives the pending marker.
        assert!(!matches!(&pending.expect, Expectation::Abstain { gap: None })
            || pending.id == "G-09");
    }

    /// Every question is owned and has a trigger: golden data is standard work
    /// and decays like any other.
    #[test]
    fn every_golden_question_is_governed() {
        for question in suite() {
            assert!(!question.owner.is_empty(), "{} has no owner", question.id);
            assert!(
                !question.review_trigger.is_empty(),
                "{} has no review trigger",
                question.id
            );
        }
    }

    /// A `pending` marker must name a registered unknown, or it is just a way
    /// of declaring the suite green.
    #[test]
    fn a_trigger_that_has_already_fired_is_caught() {
        let suite = "\
| id | Question | Expected | Owner | Review trigger |
|----|----------|----------|-------|----------------|
| G-01 | which storage engine does the project use | abstain U-5 | Greg Villa | when U-5 resolves |
| G-02 | what is the atomic unit of memory | answer D-0004 | Greg Villa | when the envelope changes |
";
        let register = "\
## Room 2

| id | Question | Trigger | Notes |
|----|----------|---------|-------|
| U-5 | ~~Storage layer~~ **Resolved 2026-08-23** → D-0019: an append-only log | — | settled |
| U-9 | Seed corpus beyond self-hosting | before the golden suite | open |

*Recorded 2026-08-23. Owner: Greg Villa.*
";
        let questions = parse_golden(suite).expect("suite parses");
        let unknowns = crate::register::parse_register(register).expect("register parses");
        let stale = stale_triggers(&questions, &unknowns);

        // The exact shape that survived a day in the real suite: an expectation
        // resting on a gap that had since been answered — unsatisfiable, and
        // passing, because the system failed to answer a question it had since
        // learned the answer to.
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].0, "G-01");
        assert!(stale[0].1.contains("U-5 is resolved"));
    }

    #[test]
    fn a_trigger_naming_an_open_question_is_not_stale() {
        let suite = "\
| id | Question | Expected | Owner | Review trigger |
|----|----------|----------|-------|----------------|
| G-01 | what licence will the engine ship under | abstain U-17 | Greg Villa | when U-17 resolves |
";
        let register = "\
## Room 2

| id | Question | Trigger | Notes |
|----|----------|---------|-------|
| U-17 | Engine license | before the repo goes public | open |

*Recorded 2026-08-23. Owner: Greg Villa.*
";
        let questions = parse_golden(suite).expect("suite parses");
        let unknowns = crate::register::parse_register(register).expect("register parses");
        assert!(stale_triggers(&questions, &unknowns).is_empty());
    }

    #[test]
    fn pending_markers_name_a_registered_unknown() {
        let text = std::fs::read_to_string(repo_root().join("docs/REGISTER.md")).unwrap();
        let register = crate::register::parse_register(&text).unwrap();
        for question in suite() {
            let Some(pending) = &question.pending else { continue };
            assert!(
                register.iter().any(|u| u.id == *pending),
                "{} is pending {pending}, which the register does not list",
                question.id
            );
        }
    }

    /// The build gate: a failure nothing predicted turns the suite red.
    #[test]
    fn the_suite_has_no_regressions() {
        let card = corpus().score(&suite());
        let regressions: Vec<String> = card
            .regressions()
            .iter()
            .map(|g| format!("{} {} (expected {:?})", g.question.id, g.verdict.label(), g.question.expect))
            .collect();
        assert!(regressions.is_empty(), "unpredicted failures: {regressions:?}");
    }

    /// Abstention earns passes. Without this the suite would quietly reward a
    /// system that answers everything.
    #[test]
    fn declining_to_answer_earns_a_pass() {
        let card = corpus().score(&suite());
        assert!(card.abstentions_rewarded() >= 3);
        assert!(card.abstentions_rewarded() <= card.passed());
    }

    /// The instrument must catch the failure it exists for: answering
    /// confidently where it should have declined.
    #[test]
    fn a_bluff_is_caught() {
        let configured = corpus();
        let trap = GoldenQuestion {
            id: "T-01".into(),
            // The record settles this — expecting an abstention is wrong, and
            // the suite must say so rather than passing.
            question: "why is the runtime embedded rather than a server".into(),
            expect: Expectation::Abstain { gap: None },
            owner: "test".into(),
            review_trigger: "never".into(),
            pending: None,
        };
        let card = configured.score(&[trap]);
        assert_eq!(card.graded[0].verdict, Verdict::Bluffed);
        assert_eq!(card.passed(), 0);
        assert_eq!(card.regressions().len(), 1);
        assert_eq!(card.graded[0].verdict.owner(), "retrieval: abstention");
    }

    /// And the converse: declining something the record does settle is a
    /// failure too, classified as calibration rather than recall when the
    /// right record was actually surfaced.
    #[test]
    fn a_missed_answer_is_classified_by_what_went_wrong() {
        let configured = corpus();
        let unanswerable = GoldenQuestion {
            id: "T-02".into(),
            question: "what colour is the logo".into(),
            expect: Expectation::Answer("D-0001".into()),
            owner: "test".into(),
            review_trigger: "never".into(),
            pending: None,
        };
        let card = configured.score(&[unanswerable]);
        assert_eq!(card.graded[0].verdict, Verdict::Missed);
        assert_eq!(card.graded[0].verdict.owner(), "retrieval: recall");
    }

    #[test]
    fn a_malformed_row_is_a_hard_error() {
        assert!(parse_golden("| G-99 | only three | cells |\n").is_err());
        assert!(parse_golden("| G-99 | q | nonsense | o | t |\n").is_err());
    }
}

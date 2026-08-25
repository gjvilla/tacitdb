//! A corpus the record does not describe.
//!
//! Everything the engine has been measured against so far is this project's own
//! decision records — fifty-four of them, written by the person reading the
//! results, about the system producing them. That corpus cannot answer two
//! questions it is repeatedly asked to.
//!
//! It cannot answer anything about **scale**. R-7 names 10^5–10^7 nodes and the
//! self-hosting corpus is four orders short, so the document-frequency
//! statistics, the index growth, the replay cost and the accumulated dead slots
//! (U-18, U-24, U-25) have all been reasoned about and never once observed.
//!
//! And it cannot grade retrieval **honestly**, because it describes its own
//! grading. A record explaining why a question fails contains that question's
//! rarest words and then ranks for it; two such records went in during a single
//! commit and moved one question's reach from 0.52 to 1.00 (U-27). The rule
//! against quoting questions catches phrases and cannot catch vocabulary — a
//! record that discusses a term at all makes that term present.
//!
//! So this corpus is generated in words that cannot appear in the other one.
//! Every topic's vocabulary is pseudo-words built from the seed, which makes the
//! separation structural rather than a discipline someone has to keep: there is
//! no way to accidentally write `tamiro` into a decision record, and no way for
//! a question about `tamiro` to be answered by a record about retrieval.
//!
//! What it deliberately does not give is real language. Synthetic prose has no
//! paraphrase, no dialect and no jargon drift, so it can measure ranking,
//! filtering and cost, and it cannot measure the thing U-23 is actually about.
//! That half of U-9 stays open.

use crate::corpus::IngestError;
use std::collections::BTreeMap;
use tacit_core::{
    Author, ClaimContent, Content, Draft, EntityId, Evidence, GapContent, HypothesisContent,
    Ledger, RecordId, RetireReason, ReviewTrigger, SourceRef, VerdictAction, VerdictContent,
    WithdrawReason,
};

/// How much corpus to build.
#[derive(Debug, Clone, Copy)]
pub struct Shape {
    pub topics: usize,
    pub claims_per_topic: usize,
    pub seed: u64,
}

impl Default for Shape {
    fn default() -> Self {
        Self { topics: 24, claims_per_topic: 8, seed: 0x7ac1_7000_0000_0001 }
    }
}

impl Shape {
    /// Roughly this many claim records, spread over a sensible number of topics.
    pub fn of_size(claims: usize) -> Self {
        let topics = ((claims as f64).sqrt() as usize).max(4);
        Self { topics, claims_per_topic: (claims / topics).max(2), ..Self::default() }
    }

    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }
}

/// One subject and the words that belong to it.
#[derive(Debug, Clone)]
pub struct Topic {
    pub label: String,
    pub subject: EntityId,
    /// Words that appear in this topic and nowhere else — what a query about
    /// this topic is built from, and what makes the ground truth checkable.
    pub vocabulary: Vec<String>,
    pub promoted: Vec<RecordId>,
}

impl Topic {
    /// A question this topic and only this topic can answer.
    pub fn question(&self) -> String {
        self.vocabulary.iter().take(3).cloned().collect::<Vec<_>>().join(" ")
    }
}

/// What the generator built, so a test can assert against what it meant rather
/// than against what it happens to produce.
#[derive(Debug, Default)]
pub struct Corpus {
    pub topics: Vec<Topic>,
    pub promoted: usize,
    pub proposed: usize,
    pub rejected: usize,
    pub retired: usize,
    /// Open questions, and questions closed each way.
    pub gaps_open: Vec<RecordId>,
    pub gaps_answered: usize,
    pub gaps_withdrawn: usize,
    pub hypotheses: usize,
    /// Deliberately planted: two promoted attribute claims about one subject
    /// and attribute, with overlapping valid time (invariant 7).
    pub contradictions: Vec<(RecordId, RecordId)>,
    /// (replaced, replacement), each pair one editorial act.
    pub supersessions: Vec<(RecordId, RecordId)>,
    /// The one claim retired for having stopped being true rather than for
    /// having been replaced, so `RetireReason` is exercised in both readings.
    pub retired_outright: Option<RecordId>,
    pub records: usize,
}

/// Build a corpus into a ledger. Deterministic in `shape.seed`: the same shape
/// produces the same corpus, so a measurement can be repeated and a regression
/// can be told from a reshuffle.
pub fn generate(ledger: &mut Ledger, shape: Shape) -> Result<Corpus, IngestError> {
    let mut rng = Rng::new(shape.seed);
    let mut built = Corpus::default();

    let keeper = Author::human("Sam Okonkwo");
    let miner = Author::agent("catalogue-miner");
    let source = ledger.add_source("synthetic/catalogue.jsonl")?;

    // Words every topic draws on, so document frequency has a realistic shape:
    // a common layer everything shares and a rare layer that discriminates.
    let common: Vec<String> = (0..40).map(|_| rng.word(2)).collect();

    for t in 0..shape.topics {
        let label = format!("subject-{t:04}");
        let subject = ledger.add_entity("component", &label)?;
        let vocabulary: Vec<String> = (0..6).map(|_| rng.word(3)).collect();
        let mut topic = Topic { label, subject, vocabulary, promoted: Vec::new() };

        for c in 0..shape.claims_per_topic {
            let body = prose(&mut rng, &topic.vocabulary, &common);
            let mut draft = Draft::new(
                if c % 5 == 0 { miner.clone() } else { keeper.clone() },
                SourceRef {
                    channel: "catalogue".into(),
                    reference: Some(format!("synthetic#{}/{c}", topic.label)),
                },
                Content::Claim(ClaimContent::Text { body, about: vec![subject] }),
            );
            draft.evidence = vec![Evidence { source, span: Some(format!("line {c}")) }];
            draft.review_trigger = Some(ReviewTrigger {
                due_at: None,
                on_event: Some("when the catalogue is re-synced".into()),
            });
            let id = ledger.append(draft)?;
            built.records += 1;

            // Most reach promoted; the rest exercise every other way out.
            match c % 10 {
                0 => built.proposed += 1,
                1 => {
                    ledger.append(verdict(
                        &keeper,
                        VerdictAction::Reject { target: id },
                        "not what the catalogue says",
                    ))?;
                    built.rejected += 1;
                    built.records += 1;
                }
                2 => {
                    // Promoted, then replaced: one verdict, both transitions.
                    ledger.append(verdict(
                        &keeper,
                        VerdictAction::Promote { target: id, retiring: None },
                        "confirmed against the catalogue",
                    ))?;
                    built.records += 1;
                    let body = prose(&mut rng, &topic.vocabulary, &common);
                    let mut replacement = Draft::new(
                        keeper.clone(),
                        SourceRef {
                            channel: "catalogue".into(),
                            reference: Some(format!("synthetic#{}/{c}r", topic.label)),
                        },
                        Content::Claim(ClaimContent::Text { body, about: vec![subject] }),
                    );
                    replacement.supersedes = Some(id);
                    let new = ledger.append(replacement)?;
                    ledger.append(verdict(
                        &keeper,
                        VerdictAction::Promote { target: new, retiring: Some(id) },
                        "the catalogue was corrected",
                    ))?;
                    built.records += 2;
                    built.retired += 1;
                    built.promoted += 1;
                    built.supersessions.push((id, new));
                    topic.promoted.push(new);
                }
                _ => {
                    ledger.append(verdict(
                        &keeper,
                        VerdictAction::Promote { target: id, retiring: None },
                        "confirmed against the catalogue",
                    ))?;
                    built.promoted += 1;
                    built.records += 1;
                    topic.promoted.push(id);
                }
            }
        }

        // One open question per topic, closed every third and fourth way so the
        // gap lifecycle is exercised rather than only its first state.
        let mut gap = Draft::new(
            keeper.clone(),
            SourceRef { channel: "review".into(), reference: Some(format!("synthetic#{t}/gap")) },
            Content::Gap(GapContent {
                question: prose(&mut rng, &topic.vocabulary, &common),
                territory: vec![subject],
            }),
        );
        gap.review_trigger =
            Some(ReviewTrigger { due_at: None, on_event: Some("when the supplier answers".into()) });
        let gap = ledger.append(gap)?;
        built.records += 1;
        match t % 4 {
            0 if !topic.promoted.is_empty() => {
                ledger.append(verdict(
                    &keeper,
                    VerdictAction::Answer { gap, with_claim: topic.promoted[0] },
                    "settled by the catalogue",
                ))?;
                built.gaps_answered += 1;
                built.records += 1;
            }
            1 => {
                ledger.append(verdict(
                    &keeper,
                    VerdictAction::Withdraw { gap, reason: WithdrawReason::NoLongerRelevant },
                    "the component was discontinued",
                ))?;
                built.gaps_withdrawn += 1;
                built.records += 1;
            }
            _ => built.gaps_open.push(gap),
        }

        // Every eighth topic gets a contradiction: two promoted claims about the
        // same attribute with overlapping validity. Legal, and flagged.
        if t % 8 == 3 {
            let a = attribute(ledger, &keeper, subject, "rating", 24.0)?;
            ledger.append(verdict(
                &keeper,
                VerdictAction::Promote { target: a, retiring: None },
                "measured on the bench",
            ))?;
            let b = attribute(ledger, &keeper, subject, "rating", 31.0)?;
            ledger.append(verdict(
                &keeper,
                VerdictAction::Promote { target: b, retiring: None },
                "measured in the field",
            ))?;
            built.records += 4;
            built.promoted += 2;
            built.contradictions.push((a, b));
        }

        // And every twelfth, a dated prediction — one scored, one abandoned.
        if t % 12 == 5 {
            let h = ledger.append(Draft::new(
                keeper.clone(),
                SourceRef { channel: "review".into(), reference: Some(format!("synthetic#{t}/h")) },
                Content::Hypothesis(HypothesisContent {
                    statement: prose(&mut rng, &topic.vocabulary, &common),
                    falsifier: Some(prose(&mut rng, &topic.vocabulary, &common)),
                    score_by: jiff::Timestamp::UNIX_EPOCH + jiff::SignedDuration::from_hours(24 * 365 * 60),
                }),
            ))?;
            ledger.append(verdict(
                &keeper,
                VerdictAction::Abandon { hypothesis: h, reason: WithdrawReason::NoLongerRelevant },
                "the line was retired before the date",
            ))?;
            built.hypotheses += 1;
            built.records += 2;
        }

        // A relation into the previous topic, so the projected graph has edges
        // to traverse rather than a dust of isolated nodes.
        if let Some(previous) = built.topics.last().map(|p: &Topic| p.subject) {
            let edge = ledger.append(Draft::new(
                keeper.clone(),
                SourceRef {
                    channel: "catalogue".into(),
                    reference: Some(format!("synthetic#{t}/edge")),
                },
                Content::Claim(ClaimContent::Relation {
                    subject,
                    predicate: "depends_on".into(),
                    object: previous,
                    properties: BTreeMap::new(),
                }),
            ))?;
            ledger.append(verdict(
                &keeper,
                VerdictAction::Promote { target: edge, retiring: None },
                "the bill of materials says so",
            ))?;
            built.records += 2;
            built.promoted += 1;
        }

        built.topics.push(topic);
    }

    // One retirement that is not a supersession, so `RetireReason` is exercised
    // in both of its ordinary readings.
    let replacements: std::collections::BTreeSet<RecordId> =
        built.supersessions.iter().map(|(_, new)| *new).collect();
    let outright = built
        .topics
        .first()
        .and_then(|t| t.promoted.iter().find(|id| !replacements.contains(id)).copied());
    if let Some(id) = outright {
        ledger.append(verdict(
            &keeper,
            VerdictAction::Retire { target: id, reason: RetireReason::NoLongerTrue },
            "the supplier changed the part",
        ))?;
        built.records += 1;
        built.retired += 1;
        built.promoted -= 1;
        built.retired_outright = Some(id);
    }

    Ok(built)
}

fn verdict(author: &Author, action: VerdictAction, why: &str) -> Draft {
    Draft::new(
        author.clone(),
        SourceRef { channel: "review".into(), reference: Some("synthetic/review".into()) },
        Content::Verdict(VerdictContent { action, rationale: Some(why.to_string()) }),
    )
}

fn attribute(
    ledger: &mut Ledger,
    author: &Author,
    subject: EntityId,
    name: &str,
    value: f64,
) -> Result<RecordId, IngestError> {
    Ok(ledger.append(Draft::new(
        author.clone(),
        SourceRef { channel: "bench".into(), reference: Some("synthetic/bench".into()) },
        Content::Claim(ClaimContent::Attribute {
            subject,
            name: name.into(),
            value: tacit_core::Value::Number(value),
        }),
    ))?)
}

/// A few sentences: mostly common words, with this topic's own scattered
/// through, so document frequency has both a floor and a signal.
fn prose(rng: &mut Rng, topic: &[String], common: &[String]) -> String {
    let length = 40 + rng.below(60);
    let mut words = Vec::with_capacity(length as usize);
    for i in 0..length {
        if i % 7 == 3 {
            words.push(topic[rng.below(topic.len() as u64) as usize].clone());
        } else {
            words.push(common[rng.below(common.len() as u64) as usize].clone());
        }
    }
    words.join(" ")
}

/// xorshift64*, so a corpus is reproducible without a dependency.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next() % n }
    }

    /// A pronounceable pseudo-word of `syllables` syllables. Nothing here can
    /// collide with the project's own vocabulary, which is the point: a
    /// question about this corpus cannot be answered by a record about the
    /// engine, however the engine's records are worded.
    fn word(&mut self, syllables: usize) -> String {
        const C: &[u8] = b"bdfgklmnprstvz";
        const V: &[u8] = b"aeiou";
        let mut word = String::with_capacity(syllables * 2);
        for _ in 0..syllables {
            word.push(C[self.below(C.len() as u64) as usize] as char);
            word.push(V[self.below(V.len() as u64) as usize] as char);
        }
        word
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tacit_core::{ClaimState, GapState, Projection, Query, RecordState, TextIndex, ViewSpec};

    fn built() -> (Ledger, Corpus) {
        let mut ledger = Ledger::new();
        let corpus = generate(&mut ledger, Shape::default()).expect("generates");
        (ledger, corpus)
    }

    #[test]
    fn the_generated_corpus_is_what_the_generator_says_it_is() {
        let (ledger, corpus) = built();
        assert_eq!(ledger.log().len(), corpus.records, "nothing appended off-report");

        let promoted = ledger.promoted_claims().count();
        assert_eq!(promoted, corpus.promoted);
        assert_eq!(ledger.registered_gaps().len(), corpus.gaps_open.len());
        for gap in &corpus.gaps_open {
            assert_eq!(ledger.state_of(*gap), Some(RecordState::Gap(GapState::Registered)));
        }

        // Every planted contradiction is found, and none is invented.
        let found = ledger.contradictions();
        assert_eq!(found.len(), corpus.contradictions.len());
        assert!(!corpus.contradictions.is_empty(), "the shape plants some");

        // Retirement in both of its readings: replaced, and stopped being true.
        let outright = corpus.retired_outright.expect("one claim retired outright");
        assert_eq!(ledger.state_of(outright), Some(RecordState::Claim(ClaimState::Retired)));
        assert!(corpus.supersessions.iter().all(|(_, new)| *new != outright));

        // Each supersession is one editorial act: the replacement promoted, the
        // replaced retired, by the same verdict.
        assert!(!corpus.supersessions.is_empty());
        for (old, new) in &corpus.supersessions {
            assert_eq!(ledger.state_of(*old), Some(RecordState::Claim(ClaimState::Retired)));
            assert_ne!(Some(*new), corpus.retired_outright);
            assert_eq!(ledger.state_of(*new), Some(RecordState::Claim(ClaimState::Promoted)));
            assert_eq!(ledger.record(*new).unwrap().envelope().supersedes(), Some(*old));
            assert_eq!(ledger.history(*new)[0].id(), ledger.history(*old).last().unwrap().id());
        }
    }

    #[test]
    fn the_ratchet_holds_over_a_corpus_nobody_wrote_by_hand() {
        let (ledger, _) = built();
        // Every promoted claim got there through a human-declared verdict, on a
        // corpus generated in bulk — the property the engine exists for, tested
        // somewhere other than the twenty-odd records a person typed.
        for claim in ledger.promoted_claims() {
            let verdicts = ledger.history(claim.id());
            assert!(!verdicts.is_empty());
            for verdict in verdicts {
                assert_eq!(
                    verdict.envelope().author().kind,
                    tacit_core::AuthorKind::Human,
                    "a verdict from {:?}",
                    verdict.envelope().author()
                );
            }
        }
        // And agent-authored proposals exist to have been refused a promotion.
        assert!(ledger.records().any(|r| r.envelope().author().kind == tacit_core::AuthorKind::Agent));
    }

    #[test]
    fn a_topic_can_only_be_answered_by_its_own_records() {
        let (ledger, corpus) = built();
        let projection = Projection::rebuild(&ledger);
        let index = TextIndex::rebuild(&ledger);
        let retriever = index.retriever(&ledger, &projection, ViewSpec::now());

        // The vocabulary is generated, so the right answer is not a matter of
        // judgement — and it cannot collide with the project's own words, which
        // is what makes this corpus immune to U-27 by construction.
        for topic in corpus.topics.iter().take(8) {
            let found = retriever.retrieve(&Query::text(topic.question()));
            let top = found.items.first().expect("something matches");
            assert!(
                topic.promoted.contains(&top.record.id()),
                "{} answered by a record from elsewhere",
                topic.label
            );
        }
    }

    #[test]
    fn the_same_seed_builds_the_same_corpus() {
        let mut a = Ledger::new();
        let mut b = Ledger::new();
        let one = generate(&mut a, Shape::default()).unwrap();
        let two = generate(&mut b, Shape::default()).unwrap();
        // Ids differ — they are minted per ledger — but the corpus does not.
        assert_eq!(one.records, two.records);
        assert_eq!(one.promoted, two.promoted);
        assert_eq!(one.contradictions.len(), two.contradictions.len());
        let words = |c: &Corpus| c.topics.iter().map(|t| t.vocabulary.join(",")).collect::<Vec<_>>();
        assert_eq!(words(&one), words(&two));

        let mut c = Ledger::new();
        let other = generate(&mut c, Shape::default().with_seed(99)).unwrap();
        assert_ne!(words(&one), words(&other), "a different seed is a different corpus");
    }
}

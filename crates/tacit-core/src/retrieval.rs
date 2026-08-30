//! Retrieval as one plan (design/001 §7): candidates, filter, expansion,
//! fusion, and budget in a single call, with abstention as a first-class
//! outcome.
//!
//! Three things here are deliberate.
//!
//! **The filter runs during scoring, not after** (R-1). Every posting is
//! checked against the view before it contributes, so a scoped query never
//! degrades to scanning an oversized candidate set and discarding most of it.
//! Admission is [`GraphView::admits_record`] — the same predicate the
//! projected graph uses, because retrieval and traversal disagreeing about
//! what a view contains is a bug waiting to happen.
//!
//! **Fusion is a stage even with one ranker** (R-2). Lexical is the only
//! candidate source today; the shape exists so vector candidates join the same
//! plan rather than being blended by an application afterwards.
//!
//! **Abstention is an outcome, not an absence** (R-10). A query whose
//! territory meets a registered gap returns that gap, so the honest answer
//! "this is a known open question" is a retrieval result rather than an
//! application heuristic.
//!
//! Like the projection, [`TextIndex`] is a derived artifact: a monotone fold
//! over the log holding no view parameters, rebuildable at any time, never
//! authoritative.

use crate::content::{ClaimContent, Content};
use crate::embedding::{Embedder, VectorIndex};
use crate::id::{EntityId, RecordId};
use crate::ledger::Ledger;
use crate::projection::{GraphView, Projection, ViewSpec};
use crate::record::Record;
use std::collections::{BTreeMap, BTreeSet};

// ── Tokenization ────────────────────────────────────────────────────────────

/// Lowercase, split on anything that is not alphanumeric. No stemming: that is
/// a language commitment this engine has not made.
pub fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| fold(&t.to_lowercase()))
        .collect()
}

/// Land a word and its plural in the same bucket.
///
/// A *collision* function, not a linguistic one: it does not need to produce
/// English, only to produce the same string for "key" and "keys". `does`
/// becomes `doe` and that is fine, because the document says `doe` too.
///
/// Deliberately small, and deliberately only plurals. Suffixes like `-ing` and
/// `-ed` cannot be stripped consistently without restoring the elided `e`
/// ("promoting" and "promote" must meet), which is the whole of a real stemmer
/// and a much larger commitment to English than this earns. What is here is
/// the case that was measured: a query asking about signing *keys* scored the
/// record that says *key* seven times at rank nineteen, because the single
/// most discriminating term in the question matched nothing at all.
///
/// This is an English crutch, like the stopword list beside it, and both are
/// the same bet: that morphology is cheaper to handle badly than to handle
/// with a model. Recorded in U-23 rather than hidden.
fn fold(token: &str) -> String {
    let n = token.len();
    if n > 4 && token.ends_with("ies") {
        return format!("{}y", &token[..n - 3]);
    }
    if n > 4 && token.ends_with("sses") {
        return token[..n - 2].to_string();
    }
    // `ss`, `us`, `is` are not plural endings: class, status, this.
    if n > 3
        && token.ends_with('s')
        && !token.ends_with("ss")
        && !token.ends_with("us")
        && !token.ends_with("is")
    {
        return token[..n - 1].to_string();
    }
    token.to_string()
}

/// The text a record contributes to the index. Verdicts contribute nothing:
/// they are the mechanism of state change, and their provenance is read
/// through `history` rather than searched.
pub fn indexable_text(record: &Record) -> Option<String> {
    match record.content() {
        Content::Claim(ClaimContent::Attribute { name, value, .. }) => {
            Some(format!("{name} {}", value.as_search_text()))
        }
        Content::Claim(ClaimContent::Relation { predicate, .. }) => Some(predicate.clone()),
        Content::Claim(ClaimContent::Pattern { context, forces, solution, .. }) => {
            Some(format!("{context} {} {solution}", forces.join(" ")))
        }
        Content::Claim(ClaimContent::Text { body, .. }) => Some(body.clone()),
        Content::Gap(gap) => Some(gap.question.clone()),
        Content::Hypothesis(h) => {
            Some(format!("{} {}", h.statement, h.falsifier.clone().unwrap_or_default()))
        }
        Content::Verdict(_) => None,
    }
}

// ── The index ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Posting {
    record: RecordId,
    /// Which passage of the record this posting counts within. A record is
    /// indexed passage by passage (U-39, D-0044), so a term's frequency is
    /// local to the window that holds it rather than diluted across a
    /// document three thousand tokens long.
    passage: u32,
    term_frequency: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DocStats {
    /// Token length of each passage, in order.
    lengths: Vec<u32>,
    entities: Vec<EntityId>,
}

/// Tokens per indexed passage — and the default is *no limit*: a record is
/// one passage, scored whole. Passage indexing was built as U-39's predicted
/// repair, swept over both suites at six sizes, and lost or tied at every
/// one of them (D-0044): scoring a record as its best window makes titles
/// and bodies compete at comparable lengths, but a window's *coverage*
/// understates every record whose answer is spread across its document, and
/// that cost exceeded the gain on both corpora. The machinery stays, switched
/// off, exactly as the approximate vector index did (U-26): the refusal is
/// corpus-relative, `with_passage_tokens` is the door, and the indexing_sweep
/// example is the instrument that reopens the question.
const PASSAGE_TOKENS: usize = usize::MAX;

/// An inverted index over record content. A fold over the log carrying no view
/// parameters, exactly like [`Projection`] and for the same reason: nothing is
/// removed, so incremental maintenance stays monotone and equivalence to
/// rebuild is definitional.
#[derive(Debug, Clone, PartialEq)]
pub struct TextIndex {
    applied: usize,
    postings: BTreeMap<String, Vec<Posting>>,
    docs: BTreeMap<RecordId, DocStats>,
    /// Number of indexed passages — the `N` of every collection statistic
    /// here, because the scored unit is the passage.
    passages: usize,
    total_length: u64,
    passage_tokens: usize,
}

impl Default for TextIndex {
    fn default() -> Self {
        Self::empty()
    }
}

impl TextIndex {
    pub fn empty() -> Self {
        Self {
            applied: 0,
            postings: BTreeMap::new(),
            docs: BTreeMap::new(),
            passages: 0,
            total_length: 0,
            passage_tokens: PASSAGE_TOKENS,
        }
    }

    /// An empty index with another passage size — the sweep instrument's
    /// entry point, and a caller's if their prose runs longer than ours.
    /// Must be set before the first `advance`: an index is one fold, and
    /// re-slicing what was already folded would break rebuild equivalence.
    pub fn with_passage_tokens(mut self, tokens: usize) -> Self {
        debug_assert_eq!(self.applied, 0, "passage size is fixed at the first fold");
        self.passage_tokens = tokens.max(1);
        self
    }

    pub fn rebuild(ledger: &Ledger) -> Self {
        let mut index = Self::empty();
        index.advance(ledger);
        index
    }

    /// Fold the log suffix this index has not seen. A no-op when nothing was
    /// appended; returns the number of records consumed.
    pub fn advance(&mut self, ledger: &Ledger) -> usize {
        let log = ledger.log();
        debug_assert!(log.len() >= self.applied, "index advanced against a shorter log");
        let start = self.applied;
        for id in &log[start..] {
            let record = ledger.record(*id).expect("log entries resolve");
            self.step(record);
        }
        self.applied = log.len();
        self.applied - start
    }

    fn step(&mut self, record: &Record) {
        let Some(text) = indexable_text(record) else { return };
        let tokens = tokenize(&text);
        if tokens.is_empty() {
            return;
        }

        let mut lengths: Vec<u32> = Vec::new();
        for (passage, window) in tokens.chunks(self.passage_tokens).enumerate() {
            let mut counts: BTreeMap<&String, u32> = BTreeMap::new();
            for token in window {
                *counts.entry(token).or_default() += 1;
            }
            for (term, term_frequency) in counts {
                self.postings.entry(term.clone()).or_default().push(Posting {
                    record: record.id(),
                    passage: passage as u32,
                    term_frequency,
                });
            }
            lengths.push(window.len() as u32);
            self.passages += 1;
        }

        let entities = match record.content() {
            Content::Claim(claim) => claim.entity_refs(),
            Content::Gap(gap) => gap.territory.clone(),
            _ => Vec::new(),
        };
        self.total_length += tokens.len() as u64;
        self.docs.insert(record.id(), DocStats { lengths, entities });
    }

    #[doc(hidden)]
    pub fn postings_len(&self, term: &str) -> Option<usize> {
        self.postings.get(term).map(|p| p.len())
    }

    /// Indexed passages — the scored unit, of which a long record has many.
    pub fn documents(&self) -> usize {
        self.passages
    }

    pub fn applied(&self) -> usize {
        self.applied
    }

    fn average_length(&self) -> f64 {
        if self.passages == 0 {
            return 0.0;
        }
        self.total_length as f64 / self.passages as f64
    }

    /// Pair the index with a ledger, a projection and a view.
    pub fn retriever<'a>(
        &'a self,
        ledger: &'a Ledger,
        projection: &'a Projection,
        spec: ViewSpec,
    ) -> Retriever<'a> {
        Retriever {
            index: self,
            ledger,
            projection,
            view: projection.view(ledger, spec),
            vectors: None,
        }
    }
}

// ── Query ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Out,
    In,
    Both,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expansion {
    pub hops: u8,
    /// Empty means any predicate.
    pub predicates: Vec<String>,
    pub direction: Direction,
}

impl Expansion {
    pub fn hops(hops: u8) -> Self {
        Self { hops, predicates: Vec::new(), direction: Direction::Both }
    }
}

/// How much of the vector index a query may skip.
///
/// `Exact` reads every admitted vector, which is correct and linear in the
/// index. `Neighbourhoods` visits the signature the query hashes into and the
/// rings around it, stopping once it has enough candidates the view admits — so
/// a filtered search narrows the traversal rather than discarding results after
/// it (R-1). What it read is always reported as `Retrieved::scanned`, because
/// an approximation nobody can see the size of is one nobody can judge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    Exact,
    Neighbourhoods { want: usize, max_buckets: usize },
}

/// How several candidate rankings combine. Reciprocal rank fusion is the
/// default because it needs no score calibration between rankers — which
/// matters the moment a vector ranker joins a lexical one.
#[derive(Debug, Clone, PartialEq)]
pub enum Fusion {
    Rrf { k: f64 },
    /// One weight per ranking, applied to normalized scores. Measured on both
    /// suites 2026-08-30 and refused as a default: normalizing lets the
    /// sharper-scored ranker swamp the other, which cost every question the
    /// vector ranker was earning.
    Weighted(Vec<f64>),
}

impl Default for Fusion {
    /// `k = 0`, and the zero is a statement, not a tuning (U-41, D-0040).
    ///
    /// The literature's k=60 exists to blunt any single ranking's top ranks
    /// across a large ensemble. With exactly two rankers it inverts the
    /// evidence: at k=60 a record at rank 0 of one list — held there by a
    /// score margin rank fusion never sees — loses to a record at ranks 1 and
    /// 2, because 1/61 < 1/62 + 1/63. Zero is the only value in the family
    /// where a first place beats a middling pair (1 > 1/2 + 1/3): first place
    /// is treated as evidence and depth is not, which matches what was
    /// measured of both rankers here — the lexical top is precise, the vector
    /// top rescues spellings and paraphrase, and both tails are noise.
    /// Swept over both suites before being believed: k ≤ 10 recovers a
    /// champion the old default lost and moves nothing else; k = 0 is chosen
    /// because k = 1 already re-inverts the champion case and passed the
    /// suite only by a lucky vector rank.
    fn default() -> Self {
        Fusion::Rrf { k: 0.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Budget {
    pub k: usize,
    pub max_tokens: usize,
}

impl Default for Budget {
    fn default() -> Self {
        Self { k: 10, max_tokens: 4_000 }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    pub text: String,
    /// Restrict to records touching these entities. Empty means unscoped.
    pub entity_scope: Vec<EntityId>,
    pub expand: Option<Expansion>,
    pub fusion: Fusion,
    pub budget: Budget,
    /// Below this fused score, results are reported as weak rather than
    /// silently blended with confident ones. Corpus-relative by nature, which
    /// is why `min_coverage` carries most of the relevance judgment.
    pub min_score: f64,
    /// The fraction of the query's *discriminating* terms the best result must
    /// cover to count as a confident match. A score threshold alone cannot
    /// tell "answers the question" from "shares a few words with it".
    pub min_coverage: f64,
    /// The fraction of the query's discriminating weight that the corpus can
    /// speak to at all. Below it, the question is mostly made of words nobody
    /// here has ever written, and no record covering the rest of it is worth
    /// calling confident.
    pub min_known: f64,
    /// Query-side function words. Defaults to [`DEFAULT_STOPWORDS`].
    pub stopwords: Vec<String>,
    /// How many open questions to offer at most. An abstention that lists
    /// every gap in the register has not helped anyone.
    pub gap_budget: usize,
    /// How much of the vector index this query may skip.
    pub probe: Probe,
    /// Cosine similarity at or above which a vector-close record is worth
    /// offering as a possibly-relevant open question. Deliberately *not* used
    /// to confer confidence on an answer — see the note in `retrieve`.
    pub min_similarity: f32,
}

impl Query {
    pub fn text(query: impl Into<String>) -> Self {
        Self {
            text: query.into(),
            entity_scope: Vec::new(),
            expand: None,
            fusion: Fusion::default(),
            budget: Budget::default(),
            min_score: 0.0,
            min_coverage: 0.5,
            min_known: 0.5,
            probe: Probe::Exact,
            stopwords: DEFAULT_STOPWORDS.iter().map(|s| s.to_string()).collect(),
            gap_budget: 3,
            min_similarity: 0.5,
        }
    }

    pub fn scoped_to(mut self, entities: Vec<EntityId>) -> Self {
        self.entity_scope = entities;
        self
    }

    pub fn expanding(mut self, expansion: Expansion) -> Self {
        self.expand = Some(expansion);
        self
    }

    pub fn with_budget(mut self, budget: Budget) -> Self {
        self.budget = budget;
        self
    }

    pub fn with_min_score(mut self, min_score: f64) -> Self {
        self.min_score = min_score;
        self
    }

    pub fn with_min_similarity(mut self, min_similarity: f32) -> Self {
        self.min_similarity = min_similarity;
        self
    }

    pub fn with_min_coverage(mut self, min_coverage: f64) -> Self {
        self.min_coverage = min_coverage;
        self
    }

    /// Replace the query-side function-word list; pass an empty vector to
    /// disable it entirely.
    pub fn with_stopwords(mut self, stopwords: Vec<String>) -> Self {
        self.stopwords = stopwords;
        self
    }
}

// ── Result ──────────────────────────────────────────────────────────────────

/// Why a record is in the result.
#[derive(Debug, Clone, PartialEq)]
pub enum Via {
    /// Matched the query text directly.
    Lexical,
    /// Matched by vector similarity only — the signal that survives spelling
    /// and morphology a token index cannot bridge.
    Vector,
    /// Both rankings found it. The strongest evidence available here.
    Hybrid,
    /// Reached by graph expansion from a seed, with the edges traversed.
    Expanded { from: RecordId, path: Vec<RecordId> },
}

#[derive(Debug, Clone)]
pub struct Item<'a> {
    pub record: &'a Record,
    /// The fusion score that decided the ordering. Its *magnitude* is not a
    /// relevance measure — reciprocal rank fusion returns roughly `1/k` for
    /// everything by construction — so compare `relevance` instead.
    pub score: f64,
    /// The best underlying ranker score. This is what `min_score` judges.
    pub relevance: f64,
    /// How much of the question's discriminating weight *this* record covers.
    /// Published per item (U-44) because the outcome's confidence has to be
    /// read from some record, fused order chooses which one arrives first,
    /// and a consumer told only the aggregate cannot see when the second item
    /// answers more of the question than the first.
    pub coverage: f64,
    /// Cosine similarity to the query, when vector candidates are in play.
    pub similarity: f64,
    pub via: Via,
    /// The window of the record a consumer receives when the whole record
    /// would not fit its share of the budget — `None` means the full text was
    /// assembled. On a long-document corpus this is what keeps the budget from
    /// deciding how many answers exist (U-43): one 3,700-token document was
    /// eating a 4,000-token budget whole, so every answer below first place
    /// existed in the plan and could not leave the engine. The full record is
    /// always reachable by id; the excerpt is assembly, not truncation of the
    /// record.
    pub excerpt: Option<String>,
}

/// The confidence half of the answer. `registered_gap` is deliberately *not*
/// one of these: a gap can stand beside confident matches ("here is what is
/// promoted, and here is the open question next to it"), so it lives in its
/// own field rather than displacing the outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Results at or above `min_score`.
    Matches,
    /// Best results were below `min_score`, and are labelled as such.
    WeakMatches,
    /// The record has nothing for this query.
    None,
}

#[derive(Debug)]
pub struct Retrieved<'a> {
    pub outcome: Outcome,
    pub items: Vec<Item<'a>>,
    /// Registered gaps whose territory the query meets. An honest "I don't
    /// know, and here is the registered question".
    pub gaps: Vec<&'a Record>,
    /// Items dropped by the budget rather than by the filter, so a caller can
    /// tell truncation from absence.
    pub truncated: usize,
    /// How much of the question the *first* item covered, and how much of the
    /// question the corpus can speak to at all. The two numbers the outcome
    /// rests on, published beside it: a reader who is told "weak" and not why
    /// cannot tell a shallow answer from an unanswerable question.
    ///
    /// First, not best — measured and kept (U-44, D-0043). This field once
    /// said "best item" while the code read the first, and judging the best
    /// coverage among assembled items was tried against both suites: it
    /// manufactures confidence from whichever record covers the most words,
    /// and the record covering the most words of an unanswerable question is
    /// simply the longest one. Each item carries its own coverage; a consumer
    /// who can weigh meaning may prefer a later item, and this engine does
    /// not pretend it can.
    pub coverage: f64,
    pub known: f64,
    /// Query terms the index read as a near neighbour, as (asked, read as).
    /// Published because a search that quietly answers a different question
    /// than the one typed is worse than one that finds nothing.
    pub read_as: Vec<(String, String)>,
    /// Vectors examined. Equal to the index size under `Probe::Exact` and much
    /// smaller under `Probe::Neighbourhoods` — published so an approximation is
    /// something a caller can see the size of rather than infer.
    pub scanned: usize,
}

impl Retrieved<'_> {
    pub fn has_registered_gap(&self) -> bool {
        !self.gaps.is_empty()
    }

    /// Every tag that applies, in the vocabulary of design/001 §7.
    pub fn tags(&self) -> Vec<&'static str> {
        let mut tags = vec![match self.outcome {
            Outcome::Matches => "matches",
            Outcome::WeakMatches => "weak_matches",
            Outcome::None => "none",
        }];
        if self.has_registered_gap() {
            tags.push("registered_gap");
        }
        tags
    }

    /// True when the honest answer is "I don't know" — nothing confident, with
    /// or without a registered question to point at.
    pub fn is_abstention(&self) -> bool {
        self.outcome != Outcome::Matches
    }
}

// ── The retriever ───────────────────────────────────────────────────────────

/// Function words carry no topic. They are dropped from *queries* only — the
/// index keeps every token — because document frequency cannot identify them
/// on a corpus like this one: in a small technical record "does" and "use" are
/// rare, so IDF rewards them, and a question's grammar outranks its subject.
///
/// This is an English default and a deliberate crutch. Callers with another
/// language, or with vector candidates carrying the semantic load, should
/// replace it via [`Query::with_stopwords`].
pub const DEFAULT_STOPWORDS: &[&str] = &[
    "a", "about", "an", "and", "any", "are", "as", "at", "be", "been", "but", "by", "can", "did",
    "do", "does", "for", "from", "get", "give", "had", "has", "have", "how", "i", "if", "in",
    "into", "is", "it", "its", "make", "may", "me", "my", "no", "not", "of", "on", "or", "our",
    "out", "over", "should", "so", "some", "than", "that", "the", "their", "them", "then",
    "there", "these", "they", "this", "to", "up", "us", "use", "used", "was", "we", "were",
    "what", "when", "where", "which", "who", "why", "will", "with", "would", "you", "your",
];

const BM25_K1: f64 = 1.2;
const BM25_B: f64 = 0.75;
/// A term appearing in more than this fraction of the corpus discriminates
/// nothing and is dropped from scoring.
const DF_PRUNE_FRACTION: f64 = 0.5;
/// Below this many documents, document frequency is too noisy to prune on.
const DF_PRUNE_MIN_DOCS: usize = 10;
/// Gaps surface at this fraction of the match coverage bar.
const GAP_COVERAGE_RATIO: f64 = 0.5;
/// Shortest word this index will read as a misspelling of another. Below it,
/// one edit is the difference between too many unrelated words.
const NEIGHBOUR_MIN_LEN: usize = 5;

/// Whether two words differ by at most one edit — one substitution, or one
/// letter inserted or removed. Exact for a distance of one and cheaper than a
/// general edit distance, which is all this needs to decide.
fn within_one_edit(a: &str, b: &str) -> bool {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    // The edit must be inside the word, never at its end. An edit at the end is
    // a suffix, and a suffix is morphology — `writer` and `write` are related
    // and are not the same word, so reading one as the other pulls in records
    // about writing for a question about writers. Measured: that substitution
    // was the only false neighbour the suite produced, and it cost the question
    // its answer. An edit inside the word is a spelling: licence/license,
    // colour/color, organise/organize all keep their last letter.
    if a.last() != b.last() {
        return false;
    }
    match a.len().abs_diff(b.len()) {
        0 => a.iter().zip(&b).filter(|(x, y)| x != y).count() == 1,
        1 => {
            // Walk together; allow the longer word exactly one skip.
            let (long, short) = if a.len() > b.len() { (&a, &b) } else { (&b, &a) };
            let (mut i, mut j, mut skipped) = (0, 0, false);
            while i < long.len() && j < short.len() {
                if long[i] == short[j] {
                    i += 1;
                    j += 1;
                } else if skipped {
                    return false;
                } else {
                    skipped = true;
                    i += 1;
                }
            }
            true
        }
        _ => false,
    }
}

/// One ranker's candidates, best first.
pub type Ranking = Vec<(RecordId, f64)>;

/// Ranked candidates plus the coverage statistics the outcome depends on.
///
/// Coverage is weighted by IDF rather than counting terms, because a result
/// that matched only "rather" has not covered a question the way one that
/// matched "storage" has. Query terms absent from the corpus entirely carry
/// the weight of a `df = 0` term, so asking about something nobody has ever
/// written about scores as the non-coverage it is.
#[derive(Debug, Default)]
struct Candidates {
    ranked: Vec<(RecordId, f64)>,
    /// Query terms the index held no posting for and read as a near neighbour,
    /// as (asked, read as). Carried out so the substitution is visible: a
    /// search that silently answers a different question than the one typed is
    /// worse than one that finds nothing.
    read_as: Vec<(String, String)>,
    matched_idf: BTreeMap<RecordId, f64>,
    /// Weight of every discriminating query term, present or not — the
    /// denominator coverage is measured against.
    total_idf: f64,
    /// The part of that weight the corpus does not contain, tracked separately
    /// so the two questions one number was answering can be told apart: "how
    /// much of this did the record cover" and "how much of it can anything
    /// here answer". The second is now published as `known`, and it is a
    /// second condition on confidence rather than a relaxation of the first —
    /// a question can only get harder to answer confidently, never easier.
    missing_idf: f64,
}

impl Candidates {
    fn coverage(&self, id: &RecordId) -> f64 {
        if self.total_idf <= 0.0 {
            return 0.0;
        }
        self.matched_idf.get(id).copied().unwrap_or(0.0) / self.total_idf
    }

    /// How much of the question the corpus can speak to at all.
    fn known(&self) -> f64 {
        if self.total_idf <= 0.0 { 0.0 } else { 1.0 - self.missing_idf / self.total_idf }
    }
}

pub struct Retriever<'a> {
    index: &'a TextIndex,
    ledger: &'a Ledger,
    projection: &'a Projection,
    view: GraphView<'a>,
    vectors: Option<(&'a VectorIndex, &'a dyn Embedder)>,
}

impl<'a> Retriever<'a> {
    /// Add vector candidates. Without this the plan runs lexical-only, which
    /// is exactly what it did before there was a second ranker — the fusion
    /// stage was built for this moment.
    pub fn with_vectors(
        mut self,
        vectors: &'a VectorIndex,
        embedder: &'a dyn Embedder,
    ) -> Self {
        self.vectors = Some((vectors, embedder));
        self
    }

    pub fn view(&self) -> GraphView<'a> {
        self.view
    }

    /// Cosine similarity against every record the view admits, checked before
    /// scoring rather than after (R-1). Exact rather than approximate: at this
    /// scale an exact scan is correct and fast, and an ANN structure is U-26.
    fn vector_candidates(&self, query: &Query) -> (Vec<(RecordId, f64)>, usize) {
        let Some((index, embedder)) = self.vectors else { return (Vec::new(), 0) };
        let probe = embedder.embed_query(&query.text);
        if probe.iter().all(|v| *v == 0.0) {
            return (Vec::new(), 0);
        }
        let scope: BTreeSet<EntityId> = query.entity_scope.iter().copied().collect();

        let admits = |id: RecordId| -> bool {
            if !scope.is_empty()
                && !self
                    .index
                    .docs
                    .get(&id)
                    .is_some_and(|d| d.entities.iter().any(|e| scope.contains(e)))
            {
                return false;
            }
            self.view.admits_record(id)
        };

        let mut scanned = 0usize;
        let mut ranked: Vec<(RecordId, f64)> = Vec::new();
        // An index built without neighbourhoods cannot be probed. Scanning it
        // is slower and right, where probing it would return nothing at all and
        // look like an empty corpus — so the fallback is to the correct answer,
        // and `scanned` says which happened.
        let plan = match query.probe {
            Probe::Neighbourhoods { .. } if !index.is_searchable() => Probe::Exact,
            other => other,
        };
        match plan {
            Probe::Exact => {
                for (id, embedded) in index.iter() {
                    scanned += 1;
                    if !admits(*id) {
                        continue;
                    }
                    let score = f64::from(embedded.similarity_to(&probe));
                    if score > 0.0 {
                        ranked.push((*id, score));
                    }
                }
            }
            Probe::Neighbourhoods { want, max_buckets } => {
                let mut buckets = 0usize;
                // Tables overlap by design, so a record can be reached more
                // than once and must only be weighed once.
                let mut seen: BTreeSet<RecordId> = BTreeSet::new();
                for bucket in index.neighbourhoods(&probe) {
                    buckets += 1;
                    for id in bucket {
                        if !seen.insert(*id) {
                            continue;
                        }
                        scanned += 1;
                        if !admits(*id) {
                            continue;
                        }
                        let Some(embedded) = index.vector(*id) else { continue };
                        let score = f64::from(embedded.similarity_to(&probe));
                        if score > 0.0 {
                            ranked.push((*id, score));
                        }
                    }
                    // Enough of what the *view* admits, not enough of what the
                    // index holds: the stopping rule is the predicate's.
                    if ranked.len() >= want || buckets >= max_buckets {
                        break;
                    }
                }
            }
        }
        ranked.sort_by(|a, b| {
            b.1.total_cmp(&a.1).then_with(|| self.log_order(a.0).cmp(&self.log_order(b.0)))
        });
        (ranked, scanned)
    }

    /// One plan: candidates, filter, expansion, fusion, budget.
    pub fn retrieve(&self, query: &Query) -> Retrieved<'a> {
        let candidates = self.lexical_candidates(query);
        let (vector, scanned) = self.vector_candidates(query);
        let rankings: Vec<Vec<(RecordId, f64)>> = if vector.is_empty() {
            vec![candidates.ranked.clone()]
        } else {
            vec![candidates.ranked.clone(), vector.clone()]
        };
        let fused = fuse(&rankings, &query.fusion);

        let raw: BTreeMap<RecordId, f64> = candidates.ranked.iter().copied().collect();
        let similarities: BTreeMap<RecordId, f64> = vector.iter().copied().collect();
        let mut items: Vec<Item<'a>> = fused
            .iter()
            .filter_map(|(id, score)| {
                self.ledger.record(*id).filter(|record| {
                    // A registered gap is the *absence* of an answer, and it
                    // has its own channel below. Leaving it here as well let it
                    // take an answer's place: a question ranked among the
                    // things that answer it, and one fewer slot for anything
                    // that does. Measured, not supposed — U-20 and U-29 were
                    // outranking the records the reader was asking for.
                    !matches!(record.content(), Content::Gap(_))
                }).map(|record| Item {
                    record,
                    score: *score,
                    excerpt: None,
                    relevance: raw.get(id).copied().unwrap_or(0.0),
                    coverage: candidates.coverage(id),
                    similarity: similarities.get(id).copied().unwrap_or(0.0),
                    via: match (raw.contains_key(id), similarities.contains_key(id)) {
                        (true, true) => Via::Hybrid,
                        (false, true) => Via::Vector,
                        _ => Via::Lexical,
                    },
                })
            })
            .collect();

        if let Some(expansion) = &query.expand {
            let seeds: Vec<RecordId> = items.iter().map(|i| i.record.id()).collect();
            items.extend(self.expand(&seeds, expansion));
        }

        // Confidence stays on the lexical signal, deliberately, and this was
        // measured rather than assumed. Over the golden questions the
        // hashing embedder's top-hit similarity ranges 0.49–0.66 for
        // answerable questions and 0.47–0.60 for unanswerable ones: the
        // distributions overlap, so no threshold separates them and any
        // vector-derived confidence would be fitted noise.
        //
        // The asymmetry that survives is the useful one. Similarity is good
        // enough to *raise a question* — an offer the reader can dismiss —
        // and not good enough to *assert an answer*. Offers get the weaker
        // signal; assertions do not. A model whose similarity does separate
        // the two can revisit this, which is what `min_similarity` is for.
        let best = items.first().map(|i| i.relevance).unwrap_or(0.0);
        let coverage =
            items.first().map(|i| candidates.coverage(&i.record.id())).unwrap_or(0.0);
        // Two conditions, because they answer different questions: coverage
        // asks whether this record answered what was asked, and `known` asks
        // whether the corpus can speak to what was asked at all. A question
        // made mostly of words nobody here has written is one to decline,
        // however well some record covers the remainder of it.
        //
        // A third clause was proposed, measured on both suites, and refused
        // (U-38, D-0042): "covered everything answerable, by a decisive
        // margin" confers confidence. The calibration instrument showed a
        // bluff at margin 1.02 beside an honest answer at 1.01, the question
        // that motivated the clause drifted out of its own precondition as
        // the corpus grew, and two questions with identical readings needed
        // opposite outcomes. Do not re-propose a rule over these quantities
        // without running `--example calibration` over both corpora first.
        let known = candidates.known();
        let outcome = if items.is_empty() {
            Outcome::None
        } else if coverage >= query.min_coverage
            && best >= query.min_score
            && known >= query.min_known
        {
            Outcome::Matches
        } else {
            Outcome::WeakMatches
        };

        let total = items.len();
        // The words an excerpt should center on: the query's own, plus any
        // spelling the index read a term as (a reader asking about "licence"
        // deserves a window around "license", not around the document's
        // opening).
        let stopwords: BTreeSet<String> = query.stopwords.iter().map(|w| fold(w)).collect();
        let mut excerpt_terms: BTreeSet<String> = tokenize(&query.text)
            .into_iter()
            .filter(|t| !stopwords.contains(t))
            .collect();
        excerpt_terms.extend(candidates.read_as.iter().map(|(_, near)| near.clone()));
        let items = self.apply_budget(items, query.budget, &excerpt_terms);
        let truncated = total - items.len();

        Retrieved {
            outcome,
            items,
            gaps: self.gaps_for(query, &candidates, &vector),
            truncated,
            coverage,
            known,
            read_as: candidates.read_as.clone(),
            scanned,
        }
    }

    /// The one index term within a single edit of a term the index does not
    /// have, when there is exactly one.
    ///
    /// A corpus written in one spelling and questioned in another loses its
    /// most discriminating word to a single letter — `licence` reaching nothing
    /// because the record says `license` (U-33). Three guards keep this from
    /// becoming a guess: it runs *only* for a term with no postings at all, so
    /// a word the corpus really has is never overridden; the words must be long
    /// enough that one edit is not most of them; and two candidates mean the
    /// index does not know which was meant, so it answers neither.
    fn nearest_term(&self, term: &str) -> Option<String> {
        if term.len() < NEIGHBOUR_MIN_LEN {
            return None;
        }
        let mut found: Option<&String> = None;
        for candidate in self.index.postings.keys() {
            if candidate.len() < NEIGHBOUR_MIN_LEN || !within_one_edit(term, candidate) {
                continue;
            }
            if found.is_some() {
                // Ambiguous. Refusing beats picking the alphabetically first.
                return None;
            }
            found = Some(candidate);
        }
        found.cloned()
    }

    /// The two candidate rankings as they stand before fusion: lexical first,
    /// vector second (empty when no vector index is attached).
    ///
    /// The golden suite grades outcomes. This is the instrument for the step
    /// before them — whether a ranker found the answer and fusion lost it, or
    /// whether nothing found it at all. Those are different faults with
    /// different fixes, and without this they look identical from outside.
    pub fn candidates(&self, query: &Query) -> (Ranking, Ranking) {
        (self.lexical_candidates(query).ranked, self.vector_candidates(query).0)
    }

    /// BM25 over the inverted index, with the view's filter applied to each
    /// posting *before* it contributes (R-1).
    fn lexical_candidates(&self, query: &Query) -> Candidates {
        // Folded on both sides, or the list stops matching the words it names.
        let stopwords: BTreeSet<String> = query.stopwords.iter().map(|w| fold(w)).collect();
        let terms: BTreeSet<String> = tokenize(&query.text)
            .into_iter()
            .filter(|t| !stopwords.contains(t))
            .collect();
        let n = self.index.passages as f64;
        let avgdl = self.index.average_length();
        if terms.is_empty() || n == 0.0 || avgdl == 0.0 {
            return Candidates::default();
        }
        let scope: BTreeSet<EntityId> = query.entity_scope.iter().copied().collect();

        // A term in most of the corpus discriminates nothing — this is the
        // standard BM25 negative-IDF cutoff, and without it "the" and "does"
        // decide the ranking. Skipped on a corpus too small for the statistic
        // to mean anything.
        let prunable = self.index.passages >= DF_PRUNE_MIN_DOCS;
        let discriminating: Vec<&String> = terms
            .iter()
            .filter(|term| {
                !prunable
                    || self
                        .index
                        .postings
                        .get(*term)
                        .is_none_or(|p| (p.len() as f64) <= n * DF_PRUNE_FRACTION)
            })
            .collect();
        if discriminating.is_empty() {
            return Candidates::default();
        }


        let mut scores: BTreeMap<(RecordId, u32), f64> = BTreeMap::new();
        let mut matched: BTreeMap<(RecordId, u32), f64> = BTreeMap::new();
        let mut total_idf = 0.0;
        let mut missing_idf = 0.0;
        let mut read_as: Vec<(String, String)> = Vec::new();
        for term in &discriminating {
            // A term the index does not have may be a spelling of one it does.
            // Tried only here, at the point the term would otherwise contribute
            // nothing at all.
            let postings = match self.index.postings.get(*term) {
                Some(postings) => Some(postings),
                None => self.nearest_term(term).and_then(|near| {
                    read_as.push(((*term).clone(), near.clone()));
                    self.index.postings.get(&near)
                }),
            };
            let Some(postings) = postings else {
                // Never written about: weigh it as a `df = 0` term would be,
                // and count it separately as well.
                //
                // It stays in the coverage denominator, and this was measured
                // rather than assumed. Taking it out is defensible — no record
                // can cover a word the corpus lacks — and it recovers one
                // underconfident answer. It also inflates coverage whenever
                // what remains is generic, and it turned G-10 from a weak miss
                // into a confident wrong answer. Across the suite the two cases
                // sit at coverage 0.77/0.60 with reach 0.63/0.64: no threshold
                // separates them,
                // so the relaxation buys one answer and costs one abstention.
                // A bluff is the worse failure, so it is not taken (U-23).
                let weight = ((n + 0.5) / 0.5 + 1.0).ln();
                total_idf += weight;
                missing_idf += weight;
                continue;
            };
            // Document frequency is a collection statistic, computed over the
            // whole index rather than the filtered subset, so that a scoped
            // query does not silently re-weight the corpus.
            let df = postings.len() as f64;
            let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
            total_idf += idf;

            for posting in postings {
                let Some(stats) = self.index.docs.get(&posting.record) else { continue };
                if !scope.is_empty() && !stats.entities.iter().any(|e| scope.contains(e)) {
                    continue;
                }
                if !self.view.admits_record(posting.record) {
                    continue;
                }
                let tf = f64::from(posting.term_frequency);
                let length = stats
                    .lengths
                    .get(posting.passage as usize)
                    .copied()
                    .unwrap_or(0);
                let norm = 1.0 - BM25_B + BM25_B * f64::from(length) / avgdl;
                let contribution = idf * (tf * (BM25_K1 + 1.0)) / (tf + BM25_K1 * norm);
                *scores.entry((posting.record, posting.passage)).or_default() += contribution;
                *matched.entry((posting.record, posting.passage)).or_default() += idf;
            }
        }

        // A record answers as its best passage (U-39, D-0044): the score and
        // the coverage both come from the one window that won, because a
        // confidence number stitched from several windows would describe a
        // document nobody reads — that is how a long record covered all of a
        // question it never answers (D-0043), and passage-local coverage is
        // what closes that door.
        let mut best: BTreeMap<RecordId, (f64, f64)> = BTreeMap::new();
        for ((record, passage), score) in scores {
            let covered = matched.get(&(record, passage)).copied().unwrap_or(0.0);
            let entry = best.entry(record).or_insert((score, covered));
            if score > entry.0 {
                *entry = (score, covered);
            }
        }
        let mut matched_idf: BTreeMap<RecordId, f64> = BTreeMap::new();
        let mut ranked: Vec<(RecordId, f64)> = Vec::new();
        for (record, (score, covered)) in best {
            matched_idf.insert(record, covered);
            ranked.push((record, score));
        }
        // Ties break on log order, so results are stable across runs.
        ranked.sort_by(|a, b| {
            b.1.total_cmp(&a.1).then_with(|| self.log_order(a.0).cmp(&self.log_order(b.0)))
        });
        Candidates { ranked, read_as, matched_idf, total_idf, missing_idf }
    }

    fn log_order(&self, id: RecordId) -> usize {
        self.ledger.log_position(id).unwrap_or(usize::MAX)
    }

    /// Walk out from the entities the seeds are about, collecting records the
    /// view admits, and remembering the edges that justified each one.
    fn expand(&self, seeds: &[RecordId], expansion: &Expansion) -> Vec<Item<'a>> {
        let seen: BTreeSet<RecordId> = seeds.iter().copied().collect();
        let mut found: Vec<Item<'a>> = Vec::new();
        let mut emitted: BTreeSet<RecordId> = seen.clone();

        for seed in seeds {
            let Some(record) = self.ledger.record(*seed) else { continue };
            let start: Vec<EntityId> = match record.content() {
                Content::Claim(claim) => claim.entity_refs(),
                Content::Gap(gap) => gap.territory.clone(),
                _ => Vec::new(),
            };

            let mut frontier: Vec<(EntityId, Vec<RecordId>)> =
                start.into_iter().map(|e| (e, Vec::new())).collect();
            let mut visited: BTreeSet<EntityId> = frontier.iter().map(|(e, _)| *e).collect();

            for _ in 0..expansion.hops {
                let mut next = Vec::new();
                for (entity, path) in &frontier {
                    let Some(node) = self.view.node(*entity) else { continue };
                    let edges = match expansion.direction {
                        Direction::Out => node.out_edges(),
                        Direction::In => node.in_edges(),
                        Direction::Both => {
                            let mut both = node.out_edges();
                            both.extend(node.in_edges());
                            both
                        }
                    };
                    for edge in edges {
                        if !expansion.predicates.is_empty()
                            && !expansion.predicates.iter().any(|p| p == edge.predicate())
                        {
                            continue;
                        }
                        let other =
                            if edge.subject() == *entity { edge.object() } else { edge.subject() };
                        if !visited.insert(other) {
                            continue;
                        }
                        let mut hop = path.clone();
                        hop.push(edge.record());
                        next.push((other, hop));
                    }
                }
                for (entity, path) in &next {
                    for record in self.view.about(*entity) {
                        if emitted.insert(record.id()) {
                            found.push(Item {
                                record,
                                // Expanded context is supporting material, not
                                // a match: it never outranks a direct hit.
                                score: 0.0,
                                relevance: 0.0,
                                coverage: 0.0,
                                similarity: 0.0,
                                via: Via::Expanded { from: *seed, path: path.clone() },
                                excerpt: None,
                            });
                        }
                    }
                }
                frontier = next;
                if frontier.is_empty() {
                    break;
                }
            }
        }
        found
    }

    /// Registered gaps the query meets — by scope if one was given, otherwise
    /// by matching the gap's own text. Gaps are indexed like every other
    /// record, which is what makes this a retrieval outcome rather than an
    /// application heuristic.
    /// The open questions this query meets.
    ///
    /// Takes the candidates the answer path already computed rather than
    /// computing them again. It used to recompute both — and since it cleared
    /// the entity scope, and this branch is only reached when there is no
    /// scope, the second pass was byte-for-byte the first. At sixty-eight
    /// thousand records that duplicate ran a full vector scan to offer at most
    /// a handful of questions, and cost as much as answering did (D-0031).
    fn gaps_for(
        &self,
        query: &Query,
        candidates: &Candidates,
        vector: &[(RecordId, f64)],
    ) -> Vec<&'a Record> {
        let scope: BTreeSet<EntityId> = query.entity_scope.iter().copied().collect();
        let mut gaps: Vec<&'a Record> = Vec::new();

        if !scope.is_empty() {
            for entity in &scope {
                for record in self.view.about(*entity) {
                    if matches!(record.content(), Content::Gap(_))
                        && !gaps.iter().any(|g| g.id() == record.id())
                    {
                        gaps.push(record);
                    }
                }
            }
            gaps.truncate(query.gap_budget);
            return gaps;
        }

        // Unscoped: a gap is offered when the question genuinely overlaps it,
        // judged by the same coverage rule that decides a confident match.
        if candidates.total_idf <= 0.0 {
            return gaps;
        }
        // A gap is an *offer* ("this may be your question"), not an assertion,
        // so it surfaces on weaker overlap than a confident match demands.
        // Requiring full match coverage would hide the registered unknown that
        // asks precisely what the questioner just asked.
        //
        // Offers are ranked by how much of the *question* they cover, not by
        // BM25: term frequency rewards a long record that repeats a common
        // word, which is the wrong instinct when choosing which open question
        // to raise.
        let gap_coverage = query.min_coverage * GAP_COVERAGE_RATIO;
        let closeness: BTreeMap<RecordId, f64> = vector.iter().copied().collect();
        let mut scored: Vec<(f64, f64, &'a Record)> = Vec::new();
        let ids: BTreeSet<RecordId> = candidates
            .ranked
            .iter()
            .map(|(id, _)| *id)
            .chain(closeness.keys().copied())
            .collect();
        for id in ids {
            let coverage = candidates.coverage(&id);
            let close = closeness.get(&id).copied().unwrap_or(0.0);
            // Either signal can raise a question worth asking.
            if coverage < gap_coverage && close < f64::from(query.min_similarity) {
                continue;
            }
            let Some(record) = self.ledger.record(id) else { continue };
            if matches!(record.content(), Content::Gap(_)) {
                scored.push((coverage, close, record));
            }
        }
        // Coverage ranks; closeness only lets a gap in the door and breaks
        // ties. This order *is* the stated rule two comments up — the first
        // version ranked by `coverage.max(close)`, which let three gaps
        // sharing no words with the question outrank the one gap covering it,
        // on the strength of a similarity this file elsewhere refuses to let
        // confer confidence (U-42, found by G-10 with the budget at three and
        // the covering gap at rank four).
        scored.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| b.1.total_cmp(&a.1)));
        gaps.extend(scored.into_iter().take(query.gap_budget).map(|(_, _, r)| r));
        gaps
    }

    /// Assemble items under the budget, excerpting any record that would not
    /// fit its share of it.
    ///
    /// The share is the budget's own arithmetic — `max_tokens / k` — and
    /// deliberately not a new constant: a budget that promises k answers
    /// within a token allowance has already said how much any one answer may
    /// take. Before this, a record was assembled whole or not at all, and on
    /// a corpus of 3,700-token documents "whole" meant the first fused item
    /// consumed the entire allowance — the ranker's second and third answers
    /// existed and could not leave the engine (U-43, found when the P-suite
    /// graded a rank-2 answer as never surfaced). Ranking is untouched:
    /// records are still scored whole, and whether they should be *indexed*
    /// in smaller pieces is U-39's question, not this one.
    fn apply_budget(
        &self,
        items: Vec<Item<'a>>,
        budget: Budget,
        terms: &BTreeSet<String>,
    ) -> Vec<Item<'a>> {
        let share = (budget.max_tokens / budget.k.max(1)).max(1);
        let mut kept = Vec::new();
        let mut tokens = 0usize;
        for mut item in items {
            if kept.len() >= budget.k {
                break;
            }
            let mut cost = 0usize;
            if let Some(text) = indexable_text(item.record) {
                cost = tokenize(&text).len();
                if cost > share {
                    let window = best_window(&text, terms, share);
                    cost = tokenize(&window).len();
                    item.excerpt = Some(window);
                }
            }
            if !kept.is_empty() && tokens + cost > budget.max_tokens {
                break;
            }
            tokens += cost;
            kept.push(item);
        }
        kept
    }

    pub fn projection(&self) -> &'a Projection {
        self.projection
    }
}

/// The `share`-word window of `text` that covers the most of the question:
/// ranked by distinct query terms present, then total occurrences, then
/// earliest — so two windows mentioning the same term once each lose to one
/// window holding two different terms, and a document that never mentions the
/// query at all yields its opening, which is at least the document introducing
/// itself. Ellipses mark where the record continues; the full text is always
/// reachable from the record id, so this is assembly, not loss.
fn best_window(text: &str, terms: &BTreeSet<String>, share: usize) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() <= share {
        return words.join(" ");
    }
    let order: BTreeMap<&str, usize> =
        terms.iter().enumerate().map(|(i, t)| (t.as_str(), i)).collect();
    // Which query term each word answers to, if any — computed once, so the
    // slide below is O(words) rather than O(words × window).
    let hit: Vec<Option<usize>> = words
        .iter()
        .map(|w| tokenize(w).iter().find_map(|t| order.get(t.as_str()).copied()))
        .collect();

    let mut counts = vec![0usize; order.len()];
    let mut distinct = 0usize;
    let mut occurrences = 0usize;
    for i in hit.iter().take(share).flatten() {
        counts[*i] += 1;
        occurrences += 1;
        if counts[*i] == 1 {
            distinct += 1;
        }
    }
    let mut best_start = 0usize;
    let mut best = (distinct, occurrences);
    for start in 1..=words.len() - share {
        if let Some(i) = hit[start - 1] {
            counts[i] -= 1;
            occurrences -= 1;
            if counts[i] == 0 {
                distinct -= 1;
            }
        }
        if let Some(i) = hit[start + share - 1] {
            counts[i] += 1;
            occurrences += 1;
            if counts[i] == 1 {
                distinct += 1;
            }
        }
        if (distinct, occurrences) > best {
            best = (distinct, occurrences);
            best_start = start;
        }
    }

    let end = best_start + share;
    let mut window = String::new();
    if best_start > 0 {
        window.push_str("… ");
    }
    window.push_str(&words[best_start..end].join(" "));
    if end < words.len() {
        window.push_str(" …");
    }
    window
}

/// Combine rankings. With one input this is order-preserving, which is the
/// point: the stage is where a second ranker will land.
pub fn fuse(rankings: &[Vec<(RecordId, f64)>], fusion: &Fusion) -> Vec<(RecordId, f64)> {
    let mut totals: BTreeMap<RecordId, f64> = BTreeMap::new();
    let mut first_rank: BTreeMap<RecordId, usize> = BTreeMap::new();

    match fusion {
        Fusion::Rrf { k } => {
            for ranking in rankings {
                for (rank, (id, _)) in ranking.iter().enumerate() {
                    *totals.entry(*id).or_default() += 1.0 / (k + (rank + 1) as f64);
                    first_rank.entry(*id).or_insert(rank);
                }
            }
        }
        Fusion::Weighted(weights) => {
            for (i, ranking) in rankings.iter().enumerate() {
                let weight = weights.get(i).copied().unwrap_or(1.0);
                let top = ranking.first().map(|(_, s)| *s).unwrap_or(0.0);
                for (rank, (id, score)) in ranking.iter().enumerate() {
                    let normalized = if top > 0.0 { score / top } else { 0.0 };
                    *totals.entry(*id).or_default() += weight * normalized;
                    first_rank.entry(*id).or_insert(rank);
                }
            }
        }
    }

    // The tie-break travels with the row rather than being looked up from
    // inside the comparator. Two map lookups per comparison is a map walked
    // `n log n` times, which is invisible at fifty candidates and most of a
    // query at twenty-five thousand — the same lesson `log_order` taught one
    // layer down (D-0031).
    let mut fused: Vec<(RecordId, f64, usize)> = totals
        .into_iter()
        .map(|(id, score)| {
            let rank = first_rank.get(&id).copied().unwrap_or(usize::MAX);
            (id, score, rank)
        })
        .collect();
    fused.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.2.cmp(&b.2)));
    fused.into_iter().map(|(id, score, _)| (id, score)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{GapContent, VerdictAction, VerdictContent};
    use crate::envelope::{Author, SourceRef};
    use crate::record::Draft;
    use crate::state::ClaimState;
    use crate::projection::StateFilter;

    struct Fixture {
        ledger: Ledger,
        torque: EntityId,
        rail: EntityId,
    }

    fn fixture() -> Fixture {
        let mut ledger = Ledger::new();
        let torque = ledger.add_entity("process", "torque check").unwrap();
        let rail = ledger.add_entity("component", "seat rail").unwrap();
        Fixture { ledger, torque, rail }
    }

    fn prose(subject: EntityId, body: &str, author: Author) -> Draft {
        Draft::new(
            author,
            SourceRef::channel("interview"),
            Content::Claim(ClaimContent::Text { body: body.into(), about: vec![subject] }),
        )
    }

    fn promote(target: RecordId) -> Draft {
        Draft::new(
            Author::human("Greg"),
            SourceRef::channel("huddle"),
            Content::Verdict(VerdictContent {
                action: VerdictAction::Promote { target, retiring: None },
                rationale: None,
            }),
        )
    }

    fn gap(question: &str, territory: Vec<EntityId>) -> Draft {
        Draft::new(
            Author::agent("assistant"),
            SourceRef::channel("chat"),
            Content::Gap(GapContent { question: question.into(), territory }),
        )
    }

    fn setup() -> (Ledger, EntityId, EntityId) {
        let mut f = fixture();
        let promoted = f
            .ledger
            .append(prose(f.torque, "the fastener seats at twenty four newton metres", Author::human("Maria")))
            .unwrap();
        f.ledger.append(promote(promoted)).unwrap();
        f.ledger
            .append(prose(f.rail, "the seat rail binds when the fixture is cold", Author::agent("miner")))
            .unwrap();
        (f.ledger, f.torque, f.rail)
    }

    fn retrieve<'a>(
        index: &'a TextIndex,
        ledger: &'a Ledger,
        projection: &'a Projection,
        spec: ViewSpec,
        query: &Query,
    ) -> Retrieved<'a> {
        index.retriever(ledger, projection, spec).retrieve(query)
    }

    #[test]
    fn lexical_search_finds_promoted_claims() {
        let (ledger, _, _) = setup();
        let index = TextIndex::rebuild(&ledger);
        let projection = Projection::rebuild(&ledger);
        let found = retrieve(
            &index,
            &ledger,
            &projection,
            ViewSpec::now(),
            &Query::text("fastener newton metres"),
        );
        assert_eq!(found.outcome, Outcome::Matches);
        assert_eq!(found.items.len(), 1);
        assert!(matches!(found.items[0].via, Via::Lexical));
    }

    #[test]
    fn a_word_and_its_plural_land_in_the_same_bucket() {
        // The measured case: a question about signing *keys* scored the record
        // saying *key* seven times at rank nineteen, because the single most
        // discriminating term in it matched nothing at all.
        assert_eq!(tokenize("keys"), tokenize("key"));
        assert_eq!(tokenize("verdicts"), tokenize("verdict"));
        assert_eq!(tokenize("queries"), tokenize("query"));
        assert_eq!(tokenize("classes"), tokenize("class"));

        // Endings that are not plurals stay whole.
        assert_eq!(tokenize("class"), vec!["class"]);
        assert_eq!(tokenize("status"), vec!["status"]);
        assert_eq!(tokenize("this"), vec!["this"]);
        // Short words are left alone rather than shortened into collisions.
        assert_eq!(tokenize("is"), vec!["is"]);
        assert_eq!(tokenize("as"), vec!["as"]);

        // It only has to be consistent, not linguistic: the index folds by the
        // same rule the query does, so `doe` matching `doe` is a match.
        assert_eq!(tokenize("does"), tokenize("doe"));
    }

    #[test]
    fn one_letter_inside_a_word_is_a_spelling_and_one_at_the_end_is_not() {
        // The pairs this exists for.
        assert!(within_one_edit("licence", "license"));
        assert!(within_one_edit("colour", "color"));
        assert!(within_one_edit("organise", "organize"));
        assert!(within_one_edit("behaviour", "behavior"));

        // A suffix is morphology, not spelling: `writer` and `write` are
        // related and are not the same word. This was the only false neighbour
        // the golden suite produced, and it cost a question its answer.
        assert!(!within_one_edit("writer", "write"));
        assert!(!within_one_edit("promote", "promoted"));
        assert!(!within_one_edit("store", "stores"));

        // Two edits are a different word.
        assert!(!within_one_edit("licence", "licensed"));
        assert!(!within_one_edit("ledger", "ledgers"));
    }

    #[test]
    fn a_word_the_corpus_spells_differently_is_still_reached() {
        let mut f = fixture();
        let claim = f
            .ledger
            .append(prose(f.torque, "the analyser records every organisation", Author::human("Maria")))
            .unwrap();
        f.ledger.append(promote(claim)).unwrap();
        let ledger = f.ledger;
        let index = TextIndex::rebuild(&ledger);
        let projection = Projection::rebuild(&ledger);

        // Asked in the other dialect. The record says `analyser`; the question
        // says `analyzer`, and one letter should not cost the whole term.
        let found = retrieve(
            &index,
            &ledger,
            &projection,
            ViewSpec::now(),
            &Query::text("analyzer"),
        );
        assert_eq!(found.items.len(), 1);
        assert_eq!(found.items[0].record.id(), claim);
        // Said out loud: a search that quietly answers a different question
        // than the one typed is worse than one that finds nothing.
        assert_eq!(found.read_as, vec![("analyzer".to_string(), "analyser".to_string())]);
        assert_eq!(found.known, 1.0);
    }

    #[test]
    fn a_word_the_corpus_has_is_never_read_as_another() {
        let mut f = fixture();
        let claim = f
            .ledger
            .append(prose(f.torque, "the analyser sits by the stone and the store", Author::human("Maria")))
            .unwrap();
        f.ledger.append(promote(claim)).unwrap();
        let ledger = f.ledger;
        let index = TextIndex::rebuild(&ledger);
        let projection = Projection::rebuild(&ledger);

        // Present, so nothing is substituted for it. A word the corpus really
        // has is never overridden by a neighbour.
        let exact =
            retrieve(&index, &ledger, &projection, ViewSpec::now(), &Query::text("analyser"));
        assert!(exact.read_as.is_empty());

        // `stole` sits one interior edit from both `stone` and `store`. The
        // index does not know which was meant, so it answers neither — refusing
        // beats picking the alphabetically first.
        let ambiguous =
            retrieve(&index, &ledger, &projection, ViewSpec::now(), &Query::text("stole"));
        assert!(ambiguous.read_as.is_empty(), "got {:?}", ambiguous.read_as);
    }

    #[test]
    fn a_question_the_corpus_has_no_words_for_says_so_separately() {
        let (ledger, _, _) = setup();
        let index = TextIndex::rebuild(&ledger);
        let projection = Projection::rebuild(&ledger);

        // Every discriminating word is one nobody here has written.
        let stranger = retrieve(
            &index,
            &ledger,
            &projection,
            ViewSpec::now(),
            &Query::text("sharding across geographic regions"),
        );
        assert_eq!(stranger.known, 0.0);
        assert_ne!(stranger.outcome, Outcome::Matches);

        // And a question in the corpus's own words reaches it completely. The
        // two numbers are published rather than folded together: a reader told
        // "weak" and not why cannot tell a shallow answer from an unanswerable
        // question.
        let native = retrieve(
            &index,
            &ledger,
            &projection,
            ViewSpec::now(),
            &Query::text("fastener newton metres"),
        );
        assert_eq!(native.known, 1.0);
        assert!(native.coverage > 0.5);
        assert_eq!(native.outcome, Outcome::Matches);
    }

    /// R-1: the filter runs while scoring, so an unpromoted claim never
    /// contributes — not even to be discarded afterwards.
    #[test]
    fn the_view_filter_is_applied_during_scoring() {
        let (ledger, _, rail) = setup();
        let rail_claim = ledger
            .records()
            .find(|r| indexable_text(r).is_some_and(|t| t.contains("binds")))
            .map(|r| r.id())
            .expect("the rail claim");
        let _ = rail;
        let index = TextIndex::rebuild(&ledger);
        let projection = Projection::rebuild(&ledger);
        let query = Query::text("seat rail binds cold fixture");

        let default = retrieve(&index, &ledger, &projection, ViewSpec::now(), &query);
        // The property under test is that the proposed claim never contributes,
        // which is checked directly. It used to be checked by asserting that
        // nothing at all came back — which held only because "seat" and "seats"
        // could not meet, so the *promoted* claim was invisible to this query
        // too. Folding plurals made it visible, weakly and correctly.
        assert!(
            !default.items.iter().any(|i| i.record.id() == rail_claim),
            "the rail claim is only proposed"
        );
        assert_eq!(default.outcome, Outcome::WeakMatches);

        let with_proposed = retrieve(
            &index,
            &ledger,
            &projection,
            ViewSpec::now().with_states(StateFilter::PromotedAndProposed),
            &query,
        );
        // Admitting proposed claims brings the rail claim in, and it wins on
        // its own words. The promoted claim is still there, still weakly, for
        // the "seat"/"seats" overlap.
        assert_eq!(with_proposed.outcome, Outcome::Matches);
        assert_eq!(with_proposed.items[0].record.id(), rail_claim);
        assert!(with_proposed.items.len() > default.items.len());
    }

    #[test]
    fn author_and_time_filters_reach_retrieval() {
        let (ledger, _, _) = setup();
        let index = TextIndex::rebuild(&ledger);
        let projection = Projection::rebuild(&ledger);
        let query = Query::text("fastener");

        let humans = retrieve(
            &index,
            &ledger,
            &projection,
            ViewSpec::now().by_author_kind(crate::envelope::AuthorKind::Human),
            &query,
        );
        assert_eq!(humans.items.len(), 1);

        let agents = retrieve(
            &index,
            &ledger,
            &projection,
            ViewSpec::now().by_author_kind(crate::envelope::AuthorKind::Agent),
            &query,
        );
        assert_eq!(agents.outcome, Outcome::None);
    }

    #[test]
    fn entity_scope_restricts_candidates() {
        let (ledger, torque, rail) = setup();
        let index = TextIndex::rebuild(&ledger);
        let projection = Projection::rebuild(&ledger);
        let spec = ViewSpec::now().with_states(StateFilter::PromotedAndProposed);

        let scoped = retrieve(
            &index,
            &ledger,
            &projection,
            spec,
            &Query::text("rail binds fixture").scoped_to(vec![rail]),
        );
        assert!(!scoped.items.is_empty());
        for item in &scoped.items {
            let Content::Claim(claim) = item.record.content() else { continue };
            assert!(claim.entity_refs().contains(&rail));
        }

        let other = retrieve(
            &index,
            &ledger,
            &projection,
            spec,
            &Query::text("fastener").scoped_to(vec![rail]),
        );
        assert_eq!(other.outcome, Outcome::None, "the fastener claim is about torque, not rail");

        // The same query unscoped does find it.
        let unscoped = retrieve(&index, &ledger, &projection, spec, &Query::text("fastener"));
        assert_eq!(unscoped.outcome, Outcome::Matches);
        let _ = torque;
    }

    /// R-10: a query whose territory meets an open question returns it, and it
    /// can stand beside confident matches rather than displacing them.
    #[test]
    fn registered_gaps_are_a_retrieval_outcome() {
        let (mut ledger, torque, _) = setup();
        ledger.append(gap("what torque for the rail fastener in cold weather?", vec![torque])).unwrap();
        let index = TextIndex::rebuild(&ledger);
        let projection = Projection::rebuild(&ledger);

        let found = retrieve(
            &index,
            &ledger,
            &projection,
            ViewSpec::now(),
            &Query::text("cold weather torque"),
        );
        assert!(found.has_registered_gap());
        assert!(found.tags().contains(&"registered_gap"));

        // Beside a confident match, both are reported.
        let both = retrieve(
            &index,
            &ledger,
            &projection,
            ViewSpec::now(),
            &Query::text("fastener torque").scoped_to(vec![torque]),
        );
        // Weak, and it should be. The only thing this record holds about torque
        // is an open question; the one promoted claim covers "fastener" and not
        // "torque". This read `matches` until gaps stopped occupying answer
        // slots — the gap was ranking first and its coverage was deciding how
        // confident the *answer* looked, which is a question flattering the
        // absence of an answer to it.
        assert_eq!(both.outcome, Outcome::WeakMatches);
        assert!(both.has_registered_gap());
        assert_eq!(both.tags(), vec!["weak_matches", "registered_gap"]);
        assert!(
            both.items.iter().all(|i| !matches!(i.record.content(), Content::Gap(_))),
            "a gap is never an answer"
        );
    }

    /// An answered gap stops being an open question, so it stops being an
    /// abstention the engine offers.
    #[test]
    fn answered_gaps_no_longer_abstain() {
        let (mut ledger, torque, _) = setup();
        let g = ledger.append(gap("what torque in cold weather?", vec![torque])).unwrap();
        let answer = ledger
            .append(prose(torque, "in cold weather the value is unchanged", Author::human("Maria")))
            .unwrap();
        ledger.append(promote(answer)).unwrap();

        let index = TextIndex::rebuild(&ledger);
        let projection = Projection::rebuild(&ledger);
        let query = Query::text("cold weather").scoped_to(vec![torque]);
        assert!(retrieve(&index, &ledger, &projection, ViewSpec::now(), &query).has_registered_gap());

        ledger
            .append(Draft::new(
                Author::human("Greg"),
                SourceRef::channel("huddle"),
                Content::Verdict(VerdictContent {
                    action: VerdictAction::Answer { gap: g, with_claim: answer },
                    rationale: None,
                }),
            ))
            .unwrap();
        let projection = Projection::rebuild(&ledger);
        let after = retrieve(&index, &ledger, &projection, ViewSpec::now(), &query);
        assert!(!after.has_registered_gap(), "an answered question is no longer open");
    }

    #[test]
    fn weak_matches_are_labelled_not_blended() {
        let (ledger, _, _) = setup();
        let index = TextIndex::rebuild(&ledger);
        let projection = Projection::rebuild(&ledger);
        let found = retrieve(
            &index,
            &ledger,
            &projection,
            ViewSpec::now(),
            &Query::text("fastener").with_min_score(1_000.0),
        );
        assert_eq!(found.outcome, Outcome::WeakMatches);
        assert!(found.is_abstention());
        assert!(!found.items.is_empty(), "the results are returned, but labelled");
    }

    #[test]
    fn an_empty_record_abstains() {
        let ledger = Ledger::new();
        let index = TextIndex::rebuild(&ledger);
        let projection = Projection::rebuild(&ledger);
        let found =
            retrieve(&index, &ledger, &projection, ViewSpec::now(), &Query::text("anything"));
        assert_eq!(found.outcome, Outcome::None);
        assert_eq!(found.tags(), vec!["none"]);
        assert!(found.is_abstention());
    }

    #[test]
    fn expansion_carries_the_path_that_justified_it() {
        let (mut ledger, torque, rail) = setup();
        let edge = ledger
            .append(Draft::new(
                Author::human("Greg"),
                SourceRef::channel("interview"),
                Content::Claim(ClaimContent::Relation {
                    subject: torque,
                    predicate: "applies_to".into(),
                    object: rail,
                    properties: Default::default(),
                }),
            ))
            .unwrap();
        ledger.append(promote(edge)).unwrap();
        let neighbour = ledger
            .append(prose(rail, "the rail ships pre-drilled", Author::human("Maria")))
            .unwrap();
        ledger.append(promote(neighbour)).unwrap();

        let index = TextIndex::rebuild(&ledger);
        let projection = Projection::rebuild(&ledger);
        let found = retrieve(
            &index,
            &ledger,
            &projection,
            ViewSpec::now(),
            &Query::text("fastener").expanding(Expansion::hops(1)),
        );
        let expanded: Vec<_> =
            found.items.iter().filter(|i| matches!(i.via, Via::Expanded { .. })).collect();
        assert!(!expanded.is_empty(), "the neighbour is reachable in one hop");
        let Via::Expanded { path, .. } = &expanded[0].via else { panic!() };
        assert_eq!(path, &vec![edge], "the traversed edge is reported");
    }

    #[test]
    fn the_budget_truncates_and_says_so() {
        let mut f = fixture();
        for i in 0..8 {
            let id = f
                .ledger
                .append(prose(f.torque, &format!("fastener note number {i}"), Author::human("M")))
                .unwrap();
            f.ledger.append(promote(id)).unwrap();
        }
        let index = TextIndex::rebuild(&f.ledger);
        let projection = Projection::rebuild(&f.ledger);
        let found = retrieve(
            &index,
            &f.ledger,
            &projection,
            ViewSpec::now(),
            &Query::text("fastener").with_budget(Budget { k: 3, max_tokens: 10_000 }),
        );
        assert_eq!(found.items.len(), 3);
        assert_eq!(found.truncated, 5);
    }

    #[test]
    fn verdicts_are_not_retrievable() {
        let (ledger, _, _) = setup();
        let index = TextIndex::rebuild(&ledger);
        for record in ledger.records() {
            if matches!(record.content(), Content::Verdict(_)) {
                assert!(indexable_text(record).is_none());
                assert!(!index.docs.contains_key(&record.id()));
            }
        }
    }

    /// The index is a derived artifact under the same discipline as the
    /// projection: incremental maintenance equals rebuild.
    #[test]
    fn incremental_advance_equals_rebuild() {
        let mut f = fixture();
        let mut incremental = TextIndex::empty();
        let a = f.ledger.append(prose(f.torque, "first note", Author::human("M"))).unwrap();
        incremental.advance(&f.ledger);
        f.ledger.append(promote(a)).unwrap();
        incremental.advance(&f.ledger);
        f.ledger.append(prose(f.rail, "second note", Author::agent("x"))).unwrap();
        incremental.advance(&f.ledger);
        assert_eq!(incremental, TextIndex::rebuild(&f.ledger));
        assert_eq!(incremental.advance(&f.ledger), 0);
    }

    #[test]
    fn rrf_fusion_is_order_preserving_for_one_ranking() {
        let mut ledger = Ledger::new();
        let e = ledger.add_entity("x", "x").unwrap();
        let a = ledger.append(prose(e, "alpha", Author::human("M"))).unwrap();
        let b = ledger.append(prose(e, "beta", Author::human("M"))).unwrap();
        let ranking = vec![(a, 9.0), (b, 1.0)];
        let fused = fuse(&[ranking], &Fusion::default());
        assert_eq!(fused.iter().map(|(id, _)| *id).collect::<Vec<_>>(), vec![a, b]);
    }

    /// U-41's champion case, pinned. A record one ranker holds at first place
    /// must beat a record both rankers hold at middling ranks — at k=60 it
    /// loses (1/61 < 1/62 + 1/63), which is how three measured questions were
    /// decided by depth over decisiveness.
    #[test]
    fn a_first_place_in_one_ranking_beats_a_middling_pair() {
        let mut ledger = Ledger::new();
        let e = ledger.add_entity("x", "x").unwrap();
        let champion = ledger.append(prose(e, "alpha", Author::human("M"))).unwrap();
        let middling = ledger.append(prose(e, "beta", Author::human("M"))).unwrap();
        let third = ledger.append(prose(e, "gamma", Author::human("M"))).unwrap();

        // The champion leads the first list by a wide margin and is absent
        // from the second; the middling record sits at ranks 1 and 1. What is
        // *not* asserted: that the champion beats the second list's own first
        // place — a first place there is equally strong evidence, and honoring
        // it is what lets vector candidates rescue spellings (P-01, P-15).
        let lexical = vec![(champion, 9.0), (middling, 4.0), (third, 3.9)];
        let vector = vec![(third, 0.6), (middling, 0.55)];

        let position = |fused: &[(RecordId, f64)], id: RecordId| {
            fused.iter().position(|(f, _)| *f == id).expect("present")
        };
        let fused = fuse(&[lexical.clone(), vector.clone()], &Fusion::default());
        assert!(
            position(&fused, champion) < position(&fused, middling),
            "first place is evidence; depth is not"
        );

        // The inversion the old default carried, kept as documentation: this
        // is measured behaviour, not a hypothetical.
        let old = fuse(&[lexical, vector], &Fusion::Rrf { k: 60.0 });
        assert!(
            position(&old, middling) < position(&old, champion),
            "k=60 prefers the middling pair"
        );
    }

    /// U-44, held down: each item publishes its own coverage, and the outcome
    /// is judged from the first item's — deliberately, not by oversight
    /// (D-0043). Judging the best coverage among assembled items was measured
    /// on both suites and refused: the record covering the most words of an
    /// unanswerable question is simply the longest one, so "best" manufactures
    /// confidence out of document length. The engine publishes the per-item
    /// number and declines to weigh it, because weighing it takes meaning.
    #[test]
    fn confidence_is_the_first_items_and_each_item_publishes_its_own() {
        let mut ledger = Ledger::new();
        let subject = ledger.add_entity("process", "torque").unwrap();
        // Dense: one term, repeated — BM25 rewards a short document about one
        // word, which is what puts it first.
        let dense = ledger
            .append(prose(subject, "torque torque torque torque torque", Author::human("M")))
            .unwrap();
        ledger.append(promote(dense)).unwrap();
        // Complete: covers both terms, diluted in a longer document.
        let filler = "assorted unrelated words about other matters entirely ".repeat(12);
        let complete = ledger
            .append(prose(
                subject,
                &format!("{filler} torque calibration {filler}"),
                Author::human("M"),
            ))
            .unwrap();
        ledger.append(promote(complete)).unwrap();
        // Background records, so the query terms are rare enough to matter.
        for n in 0..4 {
            let body = format!("record {n} discussing other equipment at length {filler}");
            let id = ledger.append(prose(subject, &body, Author::human("M"))).unwrap();
            ledger.append(promote(id)).unwrap();
        }

        let index = TextIndex::rebuild(&ledger);
        let projection = Projection::rebuild(&ledger);
        let found = retrieve(
            &index,
            &ledger,
            &projection,
            ViewSpec::now(),
            &Query::text("torque calibration"),
        );
        assert_eq!(found.items[0].record.id(), dense, "the dense one-term record ranks first");
        assert!(
            found.items[0].coverage < found.items[1].coverage,
            "the second item covers more of the question ({} vs {})",
            found.items[0].coverage,
            found.items[1].coverage
        );
        assert_eq!(
            found.coverage, found.items[0].coverage,
            "the outcome reads the first item, deliberately"
        );
    }

    /// U-39's predicted repair, held down in its measured position: available
    /// and off. A passage-sized index splits long records into more documents
    /// than records; the default scores each record whole, byte-for-byte the
    /// behaviour the sweep chose (D-0044).
    #[test]
    fn passage_indexing_is_available_and_not_the_default() {
        let mut ledger = Ledger::new();
        let subject = ledger.add_entity("topic", "t").unwrap();
        let long = format!("word {}", "filler ".repeat(600));
        let id = ledger.append(prose(subject, &long, Author::human("M"))).unwrap();
        ledger.append(promote(id)).unwrap();

        let whole = TextIndex::rebuild(&ledger);
        assert_eq!(whole.documents(), 1, "the default is one passage per record");

        let mut sliced = TextIndex::empty().with_passage_tokens(200);
        sliced.advance(&ledger);
        assert!(sliced.documents() > 3, "the door stays open for a caller who wants it");
    }

    /// U-43, held down: the budget assembles k answers, not one document.
    /// Three long records that all match must all arrive, each cut to its
    /// share — before this, the first consumed the allowance whole and the
    /// ranker's other answers could not leave the engine.
    #[test]
    fn the_budget_no_longer_decides_how_many_answers_exist() {
        let mut ledger = Ledger::new();
        let subject = ledger.add_entity("process", "torque").unwrap();
        let filler = "filler ".repeat(250);
        for variant in ["first", "second", "third"] {
            let body = format!("{filler} the {variant} calibration torque is thirty newton metres {filler}");
            let id = ledger.append(prose(subject, &body, Author::human("Maria"))).unwrap();
            ledger.append(promote(id)).unwrap();
        }
        let index = TextIndex::rebuild(&ledger);
        let projection = Projection::rebuild(&ledger);
        let query = Query::text("calibration torque")
            .with_budget(Budget { k: 3, max_tokens: 600 });
        let found = retrieve(&index, &ledger, &projection, ViewSpec::now(), &query);

        assert_eq!(found.items.len(), 3, "every answer the ranker found is assembled");
        assert_eq!(found.truncated, 0);
        for item in &found.items {
            let window = item.excerpt.as_deref().expect("each long record is excerpted");
            assert!(window.contains("calibration torque"), "the window centers on the question");
            assert!(window.starts_with("… ") && window.ends_with(" …"));
            assert!(
                tokenize(window).len() <= 200 + 8,
                "an excerpt stays near its share of the budget"
            );
        }
    }

    /// A record that fits its share is assembled whole; excerpting is only
    /// what the budget forces, never a default haircut.
    #[test]
    fn a_short_record_is_assembled_whole() {
        let (ledger, _, _) = setup();
        let index = TextIndex::rebuild(&ledger);
        let projection = Projection::rebuild(&ledger);
        let found = retrieve(
            &index,
            &ledger,
            &projection,
            ViewSpec::now(),
            &Query::text("fastener torque"),
        );
        assert!(!found.items.is_empty());
        assert!(found.items.iter().all(|i| i.excerpt.is_none()));
    }

    /// The window follows the index's spelling bridge: a reader asking about
    /// a word the corpus spells differently gets the window around the
    /// corpus's spelling, not the document's opening.
    #[test]
    fn an_excerpt_follows_the_spelling_the_index_read() {
        let mut ledger = Ledger::new();
        let subject = ledger.add_entity("topic", "licensing").unwrap();
        let filler = "unrelated prose about many other matters entirely ".repeat(60);
        let body = format!("{filler} the permissive licence suits the engine {filler}");
        let id = ledger.append(prose(subject, &body, Author::human("Greg"))).unwrap();
        ledger.append(promote(id)).unwrap();
        let index = TextIndex::rebuild(&ledger);
        let projection = Projection::rebuild(&ledger);
        // The corpus writes "licence"; the reader types "license".
        let query = Query::text("permissive license")
            .with_budget(Budget { k: 4, max_tokens: 400 });
        let found = retrieve(&index, &ledger, &projection, ViewSpec::now(), &query);
        assert!(!found.items.is_empty());
        let window = found.items[0].excerpt.as_deref().expect("the record is long");
        assert!(
            window.contains("permissive licence"),
            "the window found the corpus's own spelling: {window:?}"
        );
    }

    #[test]
    fn state_changes_show_up_without_reindexing() {
        let (mut ledger, torque, _) = setup();
        let claim = ledger
            .append(prose(torque, "a distinctive phrase about brackets", Author::human("M")))
            .unwrap();
        let index = TextIndex::rebuild(&ledger);
        let projection = Projection::rebuild(&ledger);
        let query = Query::text("distinctive brackets");
        assert_eq!(
            retrieve(&index, &ledger, &projection, ViewSpec::now(), &query).outcome,
            Outcome::None
        );

        ledger.append(promote(claim)).unwrap();
        // The index is untouched; only the projection advanced.
        let projection = Projection::rebuild(&ledger);
        assert_eq!(
            retrieve(&index, &ledger, &projection, ViewSpec::now(), &query).outcome,
            Outcome::Matches,
            "promotion changes retrievability with no reindex"
        );
        let _ = ClaimState::Promoted;
    }
}

#[cfg(test)]
mod vector_tests {
    use super::*;
    use crate::embedding::HashingEmbedder;
    use crate::envelope::{Author, SourceRef};
    use crate::record::Draft;
    use crate::content::{VerdictAction, VerdictContent};

    fn promoted_claim(ledger: &mut Ledger, subject: EntityId, body: &str) -> RecordId {
        let id = ledger
            .append(Draft::new(
                Author::human("Greg"),
                SourceRef::channel("interview"),
                Content::Claim(ClaimContent::Text { body: body.into(), about: vec![subject] }),
            ))
            .unwrap();
        ledger
            .append(Draft::new(
                Author::human("Greg"),
                SourceRef::channel("huddle"),
                Content::Verdict(VerdictContent {
                    action: VerdictAction::Promote { target: id, retiring: None },
                    rationale: None,
                }),
            ))
            .unwrap();
        id
    }

    fn fixture() -> (Ledger, EntityId) {
        let mut ledger = Ledger::new();
        let subject = ledger.add_entity("topic", "licensing").unwrap();
        promoted_claim(&mut ledger, subject, "the engine license will be permissive");
        (ledger, subject)
    }

    /// What the second ranker actually buys, restated after U-33 took half of
    /// it away.
    ///
    /// This once used `licence` against a record saying `license`, and the
    /// token index really could not bridge it. It can now, and better — a
    /// lexical bridge counts toward coverage, where a close vector is only ever
    /// an offer. What is left to the vector ranker is what the spelling rule
    /// deliberately refuses: a suffix, where the words are related and are not
    /// the same word.
    #[test]
    fn vectors_reach_what_the_token_index_cannot() {
        let (ledger, _) = fixture();
        let projection = Projection::rebuild(&ledger);
        let index = TextIndex::rebuild(&ledger);
        let embedder = HashingEmbedder::default();
        let vectors = crate::embedding::VectorIndex::rebuild(&ledger, &embedder);
        let query = Query::text("licensed");

        let lexical = index
            .retriever(&ledger, &projection, ViewSpec::now())
            .retrieve(&query);
        assert!(lexical.items.is_empty(), "no token reaches 'licensed'");

        let hybrid = index
            .retriever(&ledger, &projection, ViewSpec::now())
            .with_vectors(&vectors, &embedder)
            .retrieve(&query);
        assert!(!hybrid.items.is_empty(), "the vector ranker bridges the spelling");
        assert!(matches!(hybrid.items[0].via, Via::Vector | Via::Hybrid));
        assert!(hybrid.items[0].similarity > 0.0);
    }

    /// U-42, held down as a test: the gap that covers the question outranks a
    /// gap the embedder merely thinks is nearby. Closeness opens the door and
    /// breaks ties; it never decides the order — the same asymmetry the
    /// answer path already gives similarity, arrived at the same way: G-10's
    /// covering gap sat at rank four behind three gaps sharing no words with
    /// the question, and the budget of three cut it.
    #[test]
    fn gap_offers_rank_by_coverage_not_similarity() {
        let (mut ledger, subject) = fixture();
        let covering = ledger
            .append(Draft::new(
                Author::agent("assistant"),
                SourceRef::channel("chat"),
                Content::Gap(crate::content::GapContent {
                    question: "which permissive license the engine ships under".into(),
                    territory: vec![subject],
                }),
            ))
            .unwrap();
        // Shares character shapes with the query (the hashing embedder reads
        // trigrams) and none of its tokens.
        ledger
            .append(Draft::new(
                Author::agent("assistant"),
                SourceRef::channel("chat"),
                Content::Gap(crate::content::GapContent {
                    question: "licensing permissions shipping engineering".into(),
                    territory: vec![subject],
                }),
            ))
            .unwrap();

        let projection = Projection::rebuild(&ledger);
        let index = TextIndex::rebuild(&ledger);
        let embedder = HashingEmbedder::default();
        let vectors = crate::embedding::VectorIndex::rebuild(&ledger, &embedder);
        let mut query = Query::text("which license does the engine ship under");
        query.gap_budget = 1;
        let found = index
            .retriever(&ledger, &projection, ViewSpec::now())
            .with_vectors(&vectors, &embedder)
            .retrieve(&query);
        assert_eq!(
            found.gaps.first().map(|g| g.id()),
            Some(covering),
            "the budget's one slot goes to the gap that covers the question"
        );
    }

    /// The measured decision, held down as a test: similarity does not confer
    /// confidence. A vector-only hit is reported, and reported as weak.
    #[test]
    fn a_vector_only_hit_does_not_claim_a_match() {
        let (ledger, _) = fixture();
        let projection = Projection::rebuild(&ledger);
        let index = TextIndex::rebuild(&ledger);
        let embedder = HashingEmbedder::default();
        let vectors = crate::embedding::VectorIndex::rebuild(&ledger, &embedder);

        let found = index
            .retriever(&ledger, &projection, ViewSpec::now())
            .with_vectors(&vectors, &embedder)
            .retrieve(&Query::text("licensed"));
        assert_eq!(found.outcome, Outcome::WeakMatches);
        assert!(found.is_abstention(), "a close vector is an offer, not an answer");
    }

    /// R-1, restated for the approximate path: the predicate narrows the
    /// traversal rather than being applied to its results.
    #[test]
    fn probing_stops_on_what_the_view_admits_not_on_what_the_index_holds() {
        let mut ledger = Ledger::new();
        let subject = ledger.add_entity("topic", "licensing").unwrap();
        // One admitted record among many the default view will not have.
        let wanted = promoted_claim(&mut ledger, subject, "the engine license will be permissive");
        for i in 0..200 {
            ledger
                .append(Draft::new(
                    Author::agent("miner"),
                    SourceRef::channel("c"),
                    Content::Claim(ClaimContent::Text {
                        body: format!("proposal {i} about permissive engine licensing"),
                        about: vec![subject],
                    }),
                ))
                .unwrap();
        }
        let projection = Projection::rebuild(&ledger);
        let index = TextIndex::rebuild(&ledger);
        let embedder = HashingEmbedder::default();
        let vectors = crate::embedding::VectorIndex::rebuild_searchable(&ledger, &embedder);

        let found = index
            .retriever(&ledger, &projection, ViewSpec::now())
            .with_vectors(&vectors, &embedder)
            .retrieve(&Query {
                probe: Probe::Neighbourhoods { want: 5, max_buckets: 4096 },
                ..Query::text("engine license permissive")
            });

        // Everything returned is admitted — post-filtering would have handed
        // back proposals and then thrown them away, ending with nothing.
        assert!(found.items.iter().any(|i| i.record.id() == wanted));
        for item in &found.items {
            assert_eq!(
                ledger.state_of(item.record.id()),
                Some(crate::state::RecordState::Claim(crate::state::ClaimState::Promoted))
            );
        }
        // And it kept going past the five it wanted, because the stopping rule
        // counts what the view admits and most of this index is not that.
        assert!(found.scanned > 5, "scanned {}", found.scanned);
    }

    #[test]
    fn an_approximate_probe_reads_less_and_says_how_much() {
        let mut ledger = Ledger::new();
        let subject = ledger.add_entity("topic", "licensing").unwrap();
        for i in 0..300 {
            let id = promoted_claim(
                &mut ledger,
                subject,
                &format!("record {i} on permissive engine licensing and distribution"),
            );
            let _ = id;
        }
        let projection = Projection::rebuild(&ledger);
        let index = TextIndex::rebuild(&ledger);
        let embedder = HashingEmbedder::default();
        let vectors = crate::embedding::VectorIndex::rebuild_searchable(&ledger, &embedder);
        let retriever = index
            .retriever(&ledger, &projection, ViewSpec::now())
            .with_vectors(&vectors, &embedder);

        let exact = retriever.retrieve(&Query::text("permissive engine licensing"));
        let approx = retriever.retrieve(&Query {
            probe: Probe::Neighbourhoods { want: 20, max_buckets: 32 },
            ..Query::text("permissive engine licensing")
        });

        assert_eq!(exact.scanned, vectors.len(), "exact reads all of it");
        assert!(approx.scanned < exact.scanned, "{} vs {}", approx.scanned, exact.scanned);
        // Published, not inferred: an approximation nobody can see the size of
        // is one nobody can judge.
        assert!(approx.scanned > 0);
    }

    /// The safe direction to fail: an index built without neighbourhoods cannot
    /// be probed, so it is scanned. Slower and right, where probing it would
    /// return nothing and look exactly like an empty corpus.
    #[test]
    fn a_probe_asked_of_an_index_that_cannot_be_probed_falls_back_to_the_truth() {
        let (ledger, _) = fixture();
        let projection = Projection::rebuild(&ledger);
        let index = TextIndex::rebuild(&ledger);
        let embedder = HashingEmbedder::default();
        let plain = crate::embedding::VectorIndex::rebuild(&ledger, &embedder);
        assert!(!plain.is_searchable());

        let asked = Query {
            probe: Probe::Neighbourhoods { want: 1, max_buckets: 1 },
            ..Query::text("engine license")
        };
        let found = index
            .retriever(&ledger, &projection, ViewSpec::now())
            .with_vectors(&plain, &embedder)
            .retrieve(&asked);

        assert!(!found.items.is_empty(), "an unprobeable index is scanned, not skipped");
        // And it says so: everything was read, which is what exact means.
        assert_eq!(found.scanned, plain.len());
    }

    #[test]
    fn the_view_filter_still_applies_to_vector_candidates() {
        let mut ledger = Ledger::new();
        let subject = ledger.add_entity("topic", "licensing").unwrap();
        // Proposed, never promoted.
        ledger
            .append(Draft::new(
                Author::agent("miner"),
                SourceRef::channel("pipeline"),
                Content::Claim(ClaimContent::Text {
                    body: "the engine license will be permissive".into(),
                    about: vec![subject],
                }),
            ))
            .unwrap();
        let projection = Projection::rebuild(&ledger);
        let index = TextIndex::rebuild(&ledger);
        let embedder = HashingEmbedder::default();
        let vectors = crate::embedding::VectorIndex::rebuild(&ledger, &embedder);

        let default = index
            .retriever(&ledger, &projection, ViewSpec::now())
            .with_vectors(&vectors, &embedder)
            .retrieve(&Query::text("licence"));
        assert!(default.items.is_empty(), "an unpromoted claim stays out of the default view");

        let with_proposed = index
            .retriever(
                &ledger,
                &projection,
                ViewSpec::now().with_states(crate::projection::StateFilter::PromotedAndProposed),
            )
            .with_vectors(&vectors, &embedder)
            .retrieve(&Query::text("licence"));
        assert!(!with_proposed.items.is_empty());
    }

    #[test]
    fn the_vector_index_folds_like_every_other_index() {
        let (mut ledger, subject) = fixture();
        let embedder = HashingEmbedder::default();
        let mut incremental = crate::embedding::VectorIndex::empty(embedder.model_id());
        incremental.advance(&ledger, &embedder);
        promoted_claim(&mut ledger, subject, "a second claim about licensing");
        incremental.advance(&ledger, &embedder);
        assert_eq!(incremental, crate::embedding::VectorIndex::rebuild(&ledger, &embedder));
        assert_eq!(incremental.advance(&ledger, &embedder), 0);
    }

    #[test]
    #[should_panic(expected = "cannot mix model ids")]
    fn mixing_model_ids_fails_loudly() {
        let (ledger, _) = fixture();
        let first = HashingEmbedder::default();
        let mut index = crate::embedding::VectorIndex::rebuild(&ledger, &first);
        let other = HashingEmbedder::new(128, &[3]);
        index.advance(&ledger, &other);
    }
}

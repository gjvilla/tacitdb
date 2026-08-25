//! Vector candidates: the trait the engine asks for, and one implementation
//! that needs nothing from the outside world.
//!
//! **The engine never owns a model.** It owns the index and the contract; the
//! vectors come from whatever a caller plugs in. That is not architectural
//! fastidiousness — the corpus is meant to outlive any particular model, and
//! an engine that depended on a vendor would make the record hostage to it.
//! Embeddings are derived artifacts keyed by model id (design/001 §6): change
//! the model and the index rebuilds, while not one governed record moves.
//!
//! [`HashingEmbedder`] is the built-in default, and it is important to be
//! straight about what it is. It hashes character n-grams into a fixed space —
//! the hashing trick, no training, no network, fully deterministic. That makes
//! it robust to spelling and morphology in a way a token index is not
//! ("licence" and "license" share every trigram but one), which lexical matching
//! alone cannot do. It is **not** semantic. It cannot know that
//! "storage engine" and "persistence layer" are the same idea. A model that
//! can is a caller's to supply.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Where vectors come from. Implement this to plug in a real model.
pub trait Embedder {
    /// Identifies the vector space. An index built with one model id is
    /// meaningless under another, so changing it forces a rebuild rather than
    /// silently mixing spaces.
    fn model_id(&self) -> &str;

    fn dimensions(&self) -> usize;

    /// Must return a unit-length vector of exactly `dimensions()` values.
    fn embed(&self, text: &str) -> Vec<f32>;
}

/// Character n-gram hashing. Deterministic, dependency-free, and honest about
/// its ceiling: lexical robustness, not meaning.
#[derive(Debug, Clone)]
pub struct HashingEmbedder {
    dimensions: usize,
    /// n-gram sizes taken over characters, plus whole word tokens.
    grams: Vec<usize>,
    model_id: String,
}

impl Default for HashingEmbedder {
    fn default() -> Self {
        Self::new(256, &[3, 4])
    }
}

impl HashingEmbedder {
    pub fn new(dimensions: usize, grams: &[usize]) -> Self {
        let spec = grams.iter().map(usize::to_string).collect::<Vec<_>>().join("-");
        Self {
            dimensions,
            grams: grams.to_vec(),
            model_id: format!("hashing-char{spec}-d{dimensions}"),
        }
    }

    /// FNV-1a, chosen because it is short, stable across platforms and
    /// versions, and needs no dependency. A stored index depends on this
    /// staying fixed, which is what `model_id` records.
    fn hash(text: &str, seed: u64) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325 ^ seed;
        for byte in text.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash
    }

    fn features(&self, text: &str) -> Vec<String> {
        let normalized: String = text
            .chars()
            .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { ' ' })
            .collect();
        let mut features = Vec::new();
        for word in normalized.split_whitespace() {
            // The whole token, so an exact match still counts for most.
            features.push(format!("w:{word}"));
            let chars: Vec<char> = format!(" {word} ").chars().collect();
            for n in &self.grams {
                if chars.len() < *n {
                    continue;
                }
                for window in chars.windows(*n) {
                    features.push(format!("g:{}", window.iter().collect::<String>()));
                }
            }
        }
        features
    }
}

impl Embedder for HashingEmbedder {
    fn model_id(&self) -> &str {
        &self.model_id
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        let mut vector = vec![0.0f32; self.dimensions];
        for feature in self.features(text) {
            let hash = Self::hash(&feature, 0);
            let slot = (hash % self.dimensions as u64) as usize;
            // A second hash bit gives the feature a sign, so unrelated
            // features that collide tend to cancel rather than reinforce.
            let sign = if Self::hash(&feature, 0x9e37_79b9).is_multiple_of(2) { 1.0 } else { -1.0 };
            vector[slot] += sign;
        }
        normalize(&mut vector);
        vector
    }
}

/// Scale to unit length so cosine similarity is a dot product. An all-zero
/// vector (empty text) is left alone and scores zero against everything.
pub fn normalize(vector: &mut [f32]) {
    let norm: f32 = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in vector.iter_mut() {
            *value /= norm;
        }
    }
}

/// Cosine similarity of two unit vectors.
pub fn similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// A stored vector plus what produced it. The content hash is what lets a
/// rebuild skip work, and what makes a stale entry detectable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Embedded {
    pub vector: Vec<f32>,
    pub content_hash: u64,
}

/// Derived, rebuildable, never authoritative — the same posture as every other
/// index here.
/// How many hyperplanes divide the vector space. `2^BUCKET_BITS`
/// neighbourhoods: enough that a large index thins out, few enough that
/// probing a ring of them stays cheap.
const BUCKET_BITS: u32 = 12;
/// How many independent divisions of the space to keep.
///
/// One is not enough, and the arithmetic says why before any measurement does:
/// two vectors at cosine 0.6 agree on a given random bit with probability
/// `1 - arccos(0.6)/pi`, about 0.705, so they share all ten bits of a single
/// signature only 3% of the time. A true neighbour almost never lands in the
/// query's own bucket. Eight independent tables give it eight chances.
const TABLES: usize = 8;

/// A vector index, and the neighbourhoods that let a query avoid reading all
/// of it.
///
/// Each vector carries a [`BUCKET_BITS`]-bit signature: bit *i* is the sign of
/// its dot product with hyperplane *i*. Two vectors close in cosine agree on
/// most bits, so a query need only look at the signature it hashes into and
/// the ring of signatures around it (U-26).
///
/// Sign-random projection rather than a navigable graph, and the reason is the
/// invariant rather than the recall: a signature depends on its own vector and
/// nothing else, so folding a record in later produces exactly the index a
/// rebuild would. `rebuild == empty().advance()` stays definitional (D-0016),
/// and the property test that holds it down keeps working. A graph whose edges
/// depend on insertion order, or cells whose centroids move as data arrives,
/// would both have cost that.
#[derive(Debug, Clone, PartialEq)]
pub struct VectorIndex {
    applied: usize,
    model_id: String,
    vectors: BTreeMap<crate::id::RecordId, Embedded>,
    /// Per table: signature → the records carrying it, in id order.
    buckets: Vec<BTreeMap<u64, Vec<crate::id::RecordId>>>,
    /// Per table, its hyperplanes. Made on first use once the dimension is
    /// known, and derived from a fixed seed, so two indexes over one model
    /// divide the space the same way.
    planes: Vec<Vec<Vec<f32>>>,
}

impl VectorIndex {
    pub fn empty(model_id: impl Into<String>) -> Self {
        Self {
            applied: 0,
            model_id: model_id.into(),
            vectors: BTreeMap::new(),
            buckets: Vec::new(),
            planes: Vec::new(),
        }
    }

    pub fn rebuild(ledger: &crate::ledger::Ledger, embedder: &dyn Embedder) -> Self {
        let mut index = Self::empty(embedder.model_id());
        index.advance(ledger, embedder);
        index
    }

    /// Fold the log suffix this index has not seen.
    ///
    /// # Panics
    /// If the embedder's model id differs from the one the index was built
    /// with. Mixing two vector spaces in one index would produce similarity
    /// scores that mean nothing, so this fails loudly rather than quietly.
    pub fn advance(&mut self, ledger: &crate::ledger::Ledger, embedder: &dyn Embedder) -> usize {
        assert_eq!(
            self.model_id,
            embedder.model_id(),
            "a vector index cannot mix model ids; rebuild instead"
        );
        let log = ledger.log();
        let start = self.applied;
        for id in &log[start..] {
            let Some(record) = ledger.record(*id) else { continue };
            let Some(text) = crate::retrieval::indexable_text(record) else { continue };
            let content_hash = HashingEmbedder::hash(&text, 0);
            let vector = embedder.embed(&text);
            if self.planes.is_empty() && !vector.is_empty() {
                self.planes = hyperplanes(vector.len());
                self.buckets = vec![BTreeMap::new(); TABLES];
            }
            for table in 0..self.planes.len() {
                let signature = self.signature(table, &vector);
                let bucket = self.buckets[table].entry(signature).or_default();
                if let Err(at) = bucket.binary_search(id) {
                    bucket.insert(at, *id);
                }
            }
            self.vectors.insert(*id, Embedded { vector, content_hash });
        }
        self.applied = log.len();
        self.applied - start
    }

    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }

    /// Every vector in the index. Public so the cost of scanning them can be
    /// timed from outside rather than argued about.
    pub fn iter(&self) -> impl Iterator<Item = (&crate::id::RecordId, &Embedded)> {
        self.vectors.iter()
    }

    pub fn vector(&self, id: crate::id::RecordId) -> Option<&Embedded> {
        self.vectors.get(&id)
    }

    /// The signature of a vector under one table's hyperplanes.
    fn signature(&self, table: usize, vector: &[f32]) -> u64 {
        let mut bits = 0u64;
        for (i, plane) in self.planes[table].iter().enumerate() {
            let dot: f32 = vector.iter().zip(plane).map(|(v, p)| v * p).sum();
            if dot >= 0.0 {
                bits |= 1 << i;
            }
        }
        bits
    }

    /// The index's neighbourhoods for a query, closest first: the signature the
    /// query hashes into, then every signature one bit away, then two, and so
    /// on outward.
    ///
    /// Deliberately yields *candidates* and judges nothing. The caller holds
    /// the view, so the caller filters — and because it can stop when it has
    /// enough records its view admits, a filtered search narrows the traversal
    /// instead of discarding its results afterwards (R-1).
    pub fn neighbourhoods<'a>(&'a self, query: &[f32]) -> Neighbourhoods<'a> {
        let centres = (0..self.planes.len()).map(|t| self.signature(t, query)).collect();
        Neighbourhoods {
            index: self,
            centres,
            table: 0,
            radius: 0,
            flips: Vec::new(),
            done: self.planes.is_empty(),
        }
    }
}

/// Signatures at increasing Hamming distance from a query's own.
pub struct Neighbourhoods<'a> {
    index: &'a VectorIndex,
    /// One signature per table.
    centres: Vec<u64>,
    /// Tables are walked at each radius before the radius widens, so the
    /// closest neighbourhood of every division comes before the second-closest
    /// of any of them.
    table: usize,
    radius: u32,
    /// Which bits are flipped for the next signature in the current ring.
    flips: Vec<u32>,
    done: bool,
}

impl<'a> Iterator for Neighbourhoods<'a> {
    type Item = &'a [crate::id::RecordId];

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.done {
                return None;
            }
            let table = self.table;
            let mut signature = self.centres[table];
            for bit in &self.flips {
                signature ^= 1 << bit;
            }
            self.step();
            if let Some(bucket) = self.index.buckets[table].get(&signature) {
                return Some(bucket);
            }
        }
    }
}

impl Neighbourhoods<'_> {
    /// Advance to the next combination of flipped bits, widening the ring when
    /// the current one is exhausted.
    fn step(&mut self) {
        let bits = BUCKET_BITS;
        // Next table at the same ring first.
        self.table += 1;
        if self.table < self.centres.len() {
            return;
        }
        self.table = 0;
        // Odometer over strictly increasing bit positions.
        let mut i = self.flips.len();
        while i > 0 {
            i -= 1;
            if self.flips[i] < bits - (self.flips.len() - i) as u32 {
                self.flips[i] += 1;
                for j in i + 1..self.flips.len() {
                    self.flips[j] = self.flips[j - 1] + 1;
                }
                return;
            }
        }
        self.radius += 1;
        if self.radius > bits {
            self.done = true;
            return;
        }
        self.flips = (0..self.radius).collect();
    }
}

/// Hyperplanes from a fixed seed, so the same model always divides the space
/// the same way and an index built in two sittings matches one built in one.
fn hyperplanes(dimensions: usize) -> Vec<Vec<Vec<f32>>> {
    let mut state = 0x5eed_1234_9abc_def1u64;
    let mut next = move || {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        state.wrapping_mul(0x2545_F491_4F6C_DD1D)
    };
    (0..TABLES)
        .map(|_| {
            (0..BUCKET_BITS)
                .map(|_| {
                    (0..dimensions)
                        .map(|_| {
                    // Uniform in [-1, 1): the direction is what matters, and a
                    // cheap uniform draw separates the space as well as a
                    // Gaussian one at this dimension.
                            (next() >> 11) as f32 / (1u64 << 52) as f32 * 2.0 - 1.0
                        })
                        .collect()
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vectors_are_unit_length_and_deterministic() {
        let embedder = HashingEmbedder::default();
        let a = embedder.embed("the fastener seats at twenty four newton metres");
        let b = embedder.embed("the fastener seats at twenty four newton metres");
        assert_eq!(a, b, "the same text always gives the same vector");
        assert_eq!(a.len(), embedder.dimensions());
        let norm: f32 = a.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "unit length, got {norm}");
        assert!((similarity(&a, &b) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn empty_text_is_a_zero_vector_that_matches_nothing() {
        let embedder = HashingEmbedder::default();
        let empty = embedder.embed("");
        assert!(empty.iter().all(|v| *v == 0.0));
        assert_eq!(similarity(&empty, &embedder.embed("anything")), 0.0);
    }

    /// The capability this actually buys over a token index: spelling and
    /// morphology, because the variants share nearly every character n-gram.
    #[test]
    fn spelling_variants_are_close() {
        let embedder = HashingEmbedder::default();
        // Illustrated with a pair the golden suite does not use, per D-0029:
        // a phrase repeated in this repository is a phrase the corpus can rank
        // for, and source is one edit away from documentation.
        let british = embedder.embed("the analyser recorded every organisation");
        let american = embedder.embed("the analyzer recorded every organization");
        let unrelated = embedder.embed("weighted shortest paths over the instrument panel");
        assert!(
            similarity(&british, &american) > similarity(&british, &unrelated),
            "one dialect must beat an unrelated sentence"
        );
    }

    /// And the ceiling, stated as a test so nobody mistakes this for a
    /// semantic model: synonyms sharing no characters stay far apart.
    #[test]
    fn synonyms_without_shared_characters_stay_apart() {
        let embedder = HashingEmbedder::default();
        let a = embedder.embed("storage engine");
        let b = embedder.embed("persistence layer");
        assert!(
            similarity(&a, &b) < 0.3,
            "hashed n-grams cannot see meaning; a real model is a caller's to supply"
        );
    }

    #[test]
    fn a_vector_is_in_the_neighbourhood_it_hashes_into() {
        use crate::content::{ClaimContent, Content};
        use crate::envelope::{Author, SourceRef};
        use crate::record::Draft;

        let mut ledger = crate::ledger::Ledger::new();
        let subject = ledger.add_entity("topic", "t").unwrap();
        let mut ids = Vec::new();
        for i in 0..40 {
            ids.push(
                ledger
                    .append(Draft::new(
                        Author::human("G"),
                        SourceRef::channel("c"),
                        Content::Claim(ClaimContent::Text {
                            body: format!("record {i} about fasteners and torque"),
                            about: vec![subject],
                        }),
                    ))
                    .unwrap(),
            );
        }
        let embedder = HashingEmbedder::default();
        let index = VectorIndex::rebuild(&ledger, &embedder);

        // A record's own text must reach it in the very first neighbourhood
        // returned, or the traversal is not searching where it is storing.
        for id in ids.iter().take(5) {
            let text = crate::retrieval::indexable_text(ledger.record(*id).unwrap()).unwrap();
            let first = index.neighbourhoods(&embedder.embed(&text)).next().expect("a bucket");
            assert!(first.contains(id), "{id} is not in the bucket its own text hashes into");
        }
    }

    #[test]
    fn neighbourhoods_run_out_rather_than_repeating_for_ever() {
        let embedder = HashingEmbedder::default();
        let index = VectorIndex::empty(Embedder::model_id(&embedder));
        // Nothing indexed: no planes, so nothing to walk. The iterator must end
        // rather than spin.
        assert_eq!(index.neighbourhoods(&embedder.embed("anything")).count(), 0);
    }

    #[test]
    fn different_settings_are_different_model_ids() {
        assert_ne!(
            HashingEmbedder::new(256, &[3, 4]).model_id(),
            HashingEmbedder::new(128, &[3, 4]).model_id()
        );
        assert_ne!(
            HashingEmbedder::new(256, &[3, 4]).model_id(),
            HashingEmbedder::new(256, &[3]).model_id()
        );
    }
}

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
#[derive(Debug, Clone, PartialEq)]
pub struct VectorIndex {
    applied: usize,
    model_id: String,
    vectors: BTreeMap<crate::id::RecordId, Embedded>,
}

impl VectorIndex {
    pub fn empty(model_id: impl Into<String>) -> Self {
        Self { applied: 0, model_id: model_id.into(), vectors: BTreeMap::new() }
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
            self.vectors
                .insert(*id, Embedded { vector: embedder.embed(&text), content_hash });
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

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&crate::id::RecordId, &Embedded)> {
        self.vectors.iter()
    }
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
        let licence = embedder.embed("what licence will the engine ship under");
        let license = embedder.embed("engine license: Apache-2.0 versus MIT");
        let unrelated = embedder.embed("weighted shortest paths over the instrument panel");
        assert!(
            similarity(&licence, &license) > similarity(&licence, &unrelated),
            "licence/license must beat an unrelated sentence"
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

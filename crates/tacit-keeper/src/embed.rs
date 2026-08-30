//! A real embedding model behind the trait D-0020 left waiting for one.
//!
//! Opt-in (`--features real-embedder`), deliberately: the default build stays
//! dependency-free (R-4), and a model fetched over the network on first use
//! is exactly the class of landmine the default must not carry. What lives
//! here is the measurement instrument U-23 needs — whether meaning recovers
//! what words cannot — and the register decides from the numbers whether it
//! ever becomes more than that.
//!
//! Two honest limitations, recorded rather than hidden. The model reads at
//! most its context window of a document, so a 3,700-token body is embedded
//! by its opening — the passage door (`with_passage_tokens`, D-0044) is how
//! that would change if it matters. And the trait embeds queries and
//! documents through one method, so the asymmetric query prefix this model
//! family prefers is not applied; the measurement grades the model as the
//! plumbing can actually run it.

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::sync::Mutex;
use tacit_core::Embedder;

/// `BAAI/bge-small-en-v1.5` over ONNX: 384 dimensions, ~66MB, fetched from
/// Hugging Face into fastembed's cache on first use and local thereafter.
pub struct RealEmbedder {
    model: Mutex<TextEmbedding>,
    dimensions: usize,
}

impl RealEmbedder {
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::BGESmallENV15).with_show_download_progress(true),
        )?;
        // The trait promises a fixed dimensionality; ask the model rather
        // than hard-coding what its config already knows.
        let probe = model.embed(vec!["probe"], None)?;
        let dimensions = probe.first().map(Vec::len).unwrap_or(0);
        Ok(Self { model: Mutex::new(model), dimensions })
    }
}

impl Embedder for RealEmbedder {
    fn model_id(&self) -> &str {
        "BAAI/bge-small-en-v1.5"
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    /// The asymmetric half this model family trains for: questions carry a
    /// stated purpose, documents do not. U-45's second cap, lifted — the
    /// trait's default falls through for embedders with no such distinction.
    fn embed_query(&self, text: &str) -> Vec<f32> {
        self.embed(&format!(
            "Represent this sentence for searching relevant passages: {text}"
        ))
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        let mut vector = self
            .model
            .lock()
            .expect("embedder lock")
            .embed(vec![text], None)
            .ok()
            .and_then(|mut v| v.pop())
            .unwrap_or_else(|| vec![0.0; self.dimensions]);
        // The contract is a unit vector; enforce it here rather than trusting
        // the model wrapper's configuration to keep doing it.
        let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut vector {
                *v /= norm;
            }
        }
        vector
    }
}

//! Level 2: ONNX embedding model for semantic field relevance.
#![allow(
    clippy::disallowed_methods,
    reason = "standalone I/O module: fs ops for model management"
)]
//!
//! Uses `all-MiniLM-L6-v2` (FP32, ~86MB) via ONNX Runtime.
//! The model and tokenizer are expected in `~/.tokenfleet-ai/tokenless/models/`.
//!
//! On first use, call [`ensure_models()`] to download files from the project's
//! GitHub Releases.  If the models are unavailable, the compressor degrades
//! silently to Level 1 (keyword rules).

use std::{io::Read, path::Path};

use crate::EmbedderError;

/// Wrapper around the ONNX embedding model.
///
/// Load once as a global singleton; each call to [`embed`] runs in ~1ms
/// on a single CPU core.
#[cfg(feature = "onnx")]
pub(crate) struct Embedder {
    session: ort::session::Session,
    tokenizer: tokenizers::Tokenizer,
}

#[cfg(feature = "onnx")]
impl Embedder {
    /// Load the model and tokenizer from `model_dir`.
    pub(crate) fn load(model_dir: &Path) -> Result<Self, EmbedderError> {
        let model_path = model_dir.join("all-MiniLM-L6-v2.onnx");
        let tokenizer_path = model_dir.join("tokenizer.json");

        if !model_path.exists() {
            return Err(EmbedderError::ModelNotFound(model_path));
        }
        let mut builder =
            ort::session::Session::builder().map_err(|e| EmbedderError::Ort(e.to_string()))?;
        builder = builder
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
            .map_err(|e| EmbedderError::Ort(e.to_string()))?
            .with_intra_threads(2)
            .map_err(|e| EmbedderError::Ort(e.to_string()))?;
        let session = builder
            .commit_from_file(&model_path)
            .map_err(|e| EmbedderError::Ort(e.to_string()))?;

        let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| EmbedderError::TokenizerLoad(e.to_string()))?;

        Ok(Self { session, tokenizer })
    }

    /// Compute the 384-dim embedding vector for `text`.
    pub(crate) fn embed(&mut self, text: &str) -> Result<Vec<f32>, EmbedderError> {
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| EmbedderError::Tokenize(e.to_string()))?;

        let ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        let mask: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .map(|&m| m as i64)
            .collect();
        let seq_len = ids.len();

        let input_ids = ort::value::Tensor::<i64>::from_array((vec![seq_len as i64], ids.clone()))
            .map_err(|e| EmbedderError::Ort(e.to_string()))?;
        let attention_mask = ort::value::Tensor::<i64>::from_array((vec![seq_len as i64], mask))
            .map_err(|e| EmbedderError::Ort(e.to_string()))?;

        let outputs = self
            .session
            .run(ort::inputs![
                "input_ids" => input_ids,
                "attention_mask" => attention_mask,
            ])
            .map_err(|e| EmbedderError::Ort(e.to_string()))?;

        // all-MiniLM-L6-v2 outputs "sentence_embedding" as the pooled embedding
        let (_shape, data) = outputs["sentence_embedding"]
            .try_extract_tensor::<f32>()
            .map_err(|e| EmbedderError::Ort(e.to_string()))?;
        let embedding: Vec<f32> = data.to_vec();

        Ok(embedding)
    }

    /// Cosine similarity between two embedding vectors.
    #[must_use]
    pub(crate) fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.is_empty() || b.is_empty() {
            return 0.0;
        }
        let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a > 0.0 && norm_b > 0.0 {
            dot / (norm_a * norm_b)
        } else {
            0.0
        }
    }
}

/// Ensure model files are present on disk.
///
/// Downloads from GitHub Releases if files are missing.
#[cfg(feature = "onnx")]
pub(crate) fn ensure_models(model_dir: &Path) -> Result<(), EmbedderError> {
    let model_path = model_dir.join("all-MiniLM-L6-v2.onnx");
    let tokenizer_path = model_dir.join("tokenizer.json");

    if model_path.exists() && tokenizer_path.exists() {
        return Ok(());
    }

    std::fs::create_dir_all(model_dir).map_err(EmbedderError::Io)?;

    let base_url = "https://github.com/TokenFleet-AI/tokenless/releases/download/models-v1";

    download_file(&model_path, &format!("{base_url}/all-MiniLM-L6-v2.onnx"))?;
    download_file(&tokenizer_path, &format!("{base_url}/tokenizer.json"))?;

    Ok(())
}

/// Download a file from `url` and save it to `dest`.
#[cfg(feature = "onnx")]
fn download_file(dest: &Path, url: &str) -> Result<(), EmbedderError> {
    let mut response = ureq::get(url)
        .call()
        .map_err(|e| EmbedderError::Download(e.to_string()))?;

    let mut buf = Vec::with_capacity(16 * 1024 * 1024);
    response
        .body_mut()
        .as_reader()
        .read_to_end(&mut buf)
        .map_err(EmbedderError::Io)?;

    std::fs::write(dest, &buf).map_err(EmbedderError::Io)?;

    Ok(())
}

impl std::fmt::Debug for Embedder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Embedder").finish_non_exhaustive()
    }
}

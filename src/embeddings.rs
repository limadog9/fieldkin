//! Optional native ONNX embeddings for Jaccard value-set matching.
//!
//! Enable the `embeddings` Cargo feature. Construction downloads the requested
//! model when it is absent from the configured cache; matching subsequently runs
//! in Rust through ONNX Runtime. Lexical matching never initializes a model.

use std::{fmt, sync::Mutex};

pub use fastembed::{
    EmbeddingModel, InitOptionsUserDefined, TextEmbedding, TextInitOptions,
    UserDefinedEmbeddingModel,
};

use crate::{Error, algorithms::jaccard::EmbeddingProvider};

/// A real FastEmbed model exposed through the native Jaccard provider interface.
///
/// `TextInitOptions` controls model selection, cache location, inference threads,
/// tokenizer length and ONNX execution providers. The default convenience model
/// is `AllMiniLML6V2`, corresponding to Valentine's `all-MiniLM-L6-v2`.
pub struct FastEmbedProvider {
    model: Mutex<TextEmbedding>,
    batch_size: Option<usize>,
}

impl fmt::Debug for FastEmbedProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FastEmbedProvider")
            .field("batch_size", &self.batch_size)
            .finish_non_exhaustive()
    }
}

impl FastEmbedProvider {
    /// Load a native model, downloading missing files into the configured cache.
    /// `None` uses FastEmbed's batch-size default.
    pub fn new(options: TextInitOptions, batch_size: Option<usize>) -> Result<Self, Error> {
        validate_batch_size(batch_size)?;
        if options.max_length == 0 {
            return Err(Error::InvalidConfig(
                "embedding max_length must be positive".into(),
            ));
        }
        if options.intra_threads == Some(0) {
            return Err(Error::InvalidConfig(
                "embedding intra_threads must be positive".into(),
            ));
        }
        let model = TextEmbedding::try_new(options).map_err(|error| {
            Error::Algorithm(format!("cannot initialize embedding model: {error}"))
        })?;
        Self::from_model(model, batch_size)
    }

    /// Load the upstream default sentence-transformer model on the CPU, with its
    /// 256-token truncation limit.
    pub fn all_minilm_l6_v2() -> Result<Self, Error> {
        Self::new(
            TextInitOptions::new(EmbeddingModel::AllMiniLML6V2).with_max_length(256),
            None,
        )
    }

    /// Wrap an already loaded model, including models loaded from local files.
    pub fn from_model(model: TextEmbedding, batch_size: Option<usize>) -> Result<Self, Error> {
        validate_batch_size(batch_size)?;
        Ok(Self {
            model: Mutex::new(model),
            batch_size,
        })
    }
}

impl EmbeddingProvider for FastEmbedProvider {
    fn encode(&self, values: &[String]) -> Result<Vec<Vec<f64>>, Error> {
        if values.is_empty() {
            return Ok(Vec::new());
        }
        let mut model = self
            .model
            .lock()
            .map_err(|_| Error::Algorithm("embedding model lock was poisoned".into()))?;
        model
            .embed(values, self.batch_size)
            .map(|vectors| {
                vectors
                    .into_iter()
                    .map(|vector| vector.into_iter().map(f64::from).collect())
                    .collect()
            })
            .map_err(|error| Error::Algorithm(format!("embedding inference failed: {error}")))
    }
}

fn validate_batch_size(batch_size: Option<usize>) -> Result<(), Error> {
    if batch_size == Some(0) {
        return Err(Error::InvalidConfig(
            "embedding batch_size must be positive or None".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_options_fail_before_loading_or_downloading_a_model() {
        assert!(matches!(
            FastEmbedProvider::new(TextInitOptions::default(), Some(0)),
            Err(Error::InvalidConfig(_))
        ));
        let mut options = TextInitOptions::default();
        options.max_length = 0;
        assert!(matches!(
            FastEmbedProvider::new(options, None),
            Err(Error::InvalidConfig(_))
        ));
        let mut options = TextInitOptions::default();
        options.intra_threads = Some(0);
        assert!(matches!(
            FastEmbedProvider::new(options, None),
            Err(Error::InvalidConfig(_))
        ));
    }

    #[test]
    #[ignore = "downloads a real ONNX model; run explicitly to verify native inference"]
    fn real_minilm_model_encodes_and_matches_related_values() {
        use crate::algorithms::jaccard::{JaccardConfig, JaccardDistanceMatcher};
        use std::sync::Arc;

        let provider = Arc::new(FastEmbedProvider::all_minilm_l6_v2().unwrap());
        let vectors = provider
            .encode(&["a cat on a couch".into(), "a feline on a sofa".into()])
            .unwrap();
        assert_eq!(vectors.len(), 2);
        assert_eq!(vectors[0].len(), 384);
        assert!(vectors.iter().flatten().all(|value| value.is_finite()));
        let matcher = JaccardDistanceMatcher::new(JaccardConfig {
            threshold_dist: 0.5,
            ..Default::default()
        })
        .unwrap()
        .with_embedding_provider(provider);
        assert_eq!(
            matcher
                .set_similarity(&["a cat on a couch".into()], &["a feline on a sofa".into()])
                .unwrap(),
            1.0
        );
        assert_eq!(
            matcher
                .set_similarity(
                    &["a cat on a couch".into()],
                    &["financial audit regulations".into()]
                )
                .unwrap(),
            0.0
        );
    }
}

//! Persistence backends for Pete's long-term memory.
//!
//! The [`GraphRag`] type stores [`Experience`] vectors in Qdrant and text nodes
//! in Neo4j. `explain_and_embed` uses an LLM to describe sensations before they
//! are stored.

mod experience;
mod graphrag;

pub use experience::Experience;
pub use graphrag::{GraphRag, Memory};

use thiserror::Error;

/// Errors produced by the memory backends.
#[derive(Debug, Error)]
pub enum MemoryError {
    #[error(transparent)]
    Vector(#[from] qdrant_client::QdrantError),
    #[error(transparent)]
    Graph(#[from] neo4rs::Error),
}

use llm::{
    runner::{model_from_env, stream_first_sentence},
    traits::{LLMClient, LLMError},
};
use sensor::Sensation;

/// Ask an LLM to summarize and embed a [`Sensation`].
pub async fn explain_and_embed<C: LLMClient>(
    sensation: Sensation,
    llm: &C,
) -> Result<Experience, LLMError> {
    use sensor::sensation::SensationData;
    let prompt = match &sensation.data {
        Some(SensationData::Image(bytes)) => {
            let b64 = base64::encode(bytes);
            format!("Describe this image as if you are seeing it with your own eyes, using first-person language. Be specific and present-tense. <image>{}</image>", b64)
        }
        _ => format!("Summarize in one sentence: {}", sensation.how),
    };
    let model = model_from_env();
    let (_, explanation) = stream_first_sentence(llm, &model, &prompt).await?;
    let embedding = llm.embed("gemma3:embed", &explanation).await?;
    Ok(Experience::new(sensation, explanation, embedding))
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use futures_core::Stream;
    use futures_util::stream;
    use sensor::Sensation;
    use std::pin::Pin;

    struct MockLLM;

    #[async_trait]
    impl LLMClient for MockLLM {
        async fn stream_chat(
            &self,
            _model: &str,
            _prompt: &str,
        ) -> Result<Pin<Box<dyn Stream<Item = Result<String, LLMError>> + Send>>, LLMError>
        {
            Ok(Box::pin(stream::iter(vec![
                Ok("mock explanation. ".to_string()),
                Ok("second".to_string()),
            ])))
        }

        async fn embed(&self, _model: &str, _input: &str) -> Result<Vec<f32>, LLMError> {
            Ok(vec![0.1, 0.2])
        }
    }

    #[tokio::test]
    async fn creates_experience() {
        let s = Sensation::new("test", None::<String>);
        let llm = MockLLM;
        let e = explain_and_embed(s.clone(), &llm).await.unwrap();
        assert_eq!(e.explanation, "mock explanation. ");
        assert_eq!(e.embedding, vec![0.1, 0.2]);
        assert_eq!(e.sensation, s);
    }

    struct InMem {
        inner: std::sync::Mutex<Vec<Experience>>,
    }

    #[async_trait]
    impl Memory for InMem {
        async fn store(&self, exp: Experience) -> Result<(), MemoryError> {
            self.inner.lock().unwrap().push(exp);
            Ok(())
        }
    }

    #[tokio::test]
    async fn store_in_mem() {
        let store = InMem {
            inner: std::sync::Mutex::new(Vec::new()),
        };
        let exp = Experience::new(Sensation::new("hi", None::<String>), "ok", vec![1.0]);
        store.store(exp).await.unwrap();
        assert_eq!(store.inner.lock().unwrap().len(), 1);
    }
}

use async_trait::async_trait;
use psyche::Psyche;
use psyche::ling::{ChatContext, Chatter, Doer, Message, Vectorizer};
use std::sync::Arc;
use tracing::info;

use crate::ear::NoopEar;
use crate::mouth::NoopMouth;

/// Create a psyche with dummy providers for demos/tests.
pub fn dummy_psyche() -> Psyche {
    #[derive(Clone)]
    struct Dummy;

    #[async_trait]
    impl Doer for Dummy {
        async fn follow(&self, _: &str) -> anyhow::Result<String> {
            Ok("ok".into())
        }
    }

    #[async_trait]
    impl Chatter for Dummy {
        async fn chat(&self, _: ChatContext<'_>) -> anyhow::Result<psyche::ling::ChatStream> {
            Ok(Box::pin(tokio_stream::once(Ok("hi".to_string()))))
        }
    }

    #[async_trait]
    impl Vectorizer for Dummy {
        async fn vectorize(&self, _: &str) -> anyhow::Result<Vec<f32>> {
            Ok(vec![0.0])
        }
    }

    let mouth = Arc::new(NoopMouth::default());
    let ear = Arc::new(NoopEar);
    let mut psyche = Psyche::new(
        Box::new(Dummy),
        Box::new(Dummy),
        Box::new(Dummy),
        mouth,
        ear,
    );
    psyche.set_turn_limit(usize::MAX);
    info!("created dummy psyche");
    psyche
}

/// Create a psyche backed by an Ollama server.
///
/// This uses [`OllamaProvider`](psyche::ling::OllamaProvider) for all language
/// capabilities and the no-op ear and mouth implementations.
pub fn ollama_psyche(host: &str, model: &str) -> anyhow::Result<Psyche> {
    use psyche::ling::OllamaProvider;

    let narrator = OllamaProvider::new(host, model)?;
    let voice = OllamaProvider::new(host, model)?;
    let vectorizer = OllamaProvider::new(host, model)?;

    let mouth = Arc::new(NoopMouth::default());
    let ear = Arc::new(NoopEar);

    let mut psyche = Psyche::new(
        Box::new(narrator),
        Box::new(voice),
        Box::new(vectorizer),
        mouth,
        ear,
    );
    psyche.set_turn_limit(usize::MAX);
    info!(%host, %model, "created ollama psyche");
    Ok(psyche)
}

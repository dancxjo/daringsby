use async_trait::async_trait;
use psyche::ling::{Chatter, Doer, Instruction, Message, Vectorizer};
use psyche::{ContextualPrompt, Psyche};
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
        async fn follow(&self, _: Instruction) -> anyhow::Result<String> {
            Ok("ok".into())
        }
    }

    #[async_trait]
    impl Chatter for Dummy {
        async fn chat(&self, _: &str, _: &[Message]) -> anyhow::Result<psyche::ling::ChatStream> {
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
        Arc::new(psyche::NoopMemory),
        mouth,
        ear,
    );
    let wit_tx = psyche.wit_sender();
    psyche.register_observing_wit(Arc::new(psyche::VisionWit::with_debug(
        Arc::new(Dummy),
        wit_tx.clone(),
    )));
    psyche.register_observing_wit(Arc::new(psyche::FaceMemoryWit::with_debug(wit_tx)));
    psyche.set_turn_limit(usize::MAX);
    psyche
        .voice()
        .set_prompt(ContextualPrompt::new(psyche.topic_bus()));
    info!("created dummy psyche");
    psyche
}

/// Create a psyche backed by an Ollama server.
///
/// This uses [`OllamaProvider`](psyche::ling::OllamaProvider) for all language
/// capabilities and the no-op ear and mouth implementations.
pub fn ollama_psyche(
    chatter_host: &str,
    chatter_model: &str,
    wits_host: &str,
    wits_model: &str,
    embeddings_host: &str,
    embeddings_model: &str,
    qdrant_url: &str,
    neo4j_uri: &str,
    neo4j_user: &str,
    neo4j_pass: &str,
) -> anyhow::Result<Psyche> {
    use crate::LoggingMotor;
    use psyche::ling::OllamaProvider;
    use psyche::wits::{
        BasicMemory, Combobulator, CombobulatorWit, FondDuCoeur, FondDuCoeurWit, HeartWit,
        MemoryWit, Neo4jClient, QdrantClient, WillWit,
    };

    let narrator = OllamaProvider::new(chatter_host, chatter_model)?;
    let voice = OllamaProvider::new(chatter_host, chatter_model)?;
    let vectorizer = OllamaProvider::new(embeddings_host, embeddings_model)?;

    let mouth = Arc::new(NoopMouth::default());
    let ear = Arc::new(NoopEar);

    let memory = Arc::new(BasicMemory {
        vectorizer: Arc::new(OllamaProvider::new(embeddings_host, embeddings_model)?),
        qdrant: QdrantClient::new(qdrant_url.into()),
        neo4j: Arc::new(Neo4jClient::new(
            neo4j_uri.into(),
            neo4j_user.into(),
            neo4j_pass.into(),
        )),
    });

    let mut psyche = Psyche::new(
        Box::new(narrator),
        Box::new(voice.clone()),
        Box::new(vectorizer),
        memory.clone(),
        mouth,
        ear,
    );
    let wit_tx = psyche.wit_sender();
    psyche.register_observing_wit(Arc::new(psyche::VisionWit::with_debug(
        Arc::new(OllamaProvider::new(wits_host, wits_model)?),
        wit_tx.clone(),
    )));
    psyche.register_observing_wit(Arc::new(psyche::FaceMemoryWit::with_debug(wit_tx.clone())));
    psyche.register_typed_wit(Arc::new(CombobulatorWit::new(Combobulator::with_debug(
        Box::new(OllamaProvider::new(wits_host, wits_model)?),
        wit_tx.clone(),
    ))));
    psyche.register_typed_wit(Arc::new(WillWit::with_debug(
        psyche.topic_bus(),
        Arc::new(OllamaProvider::new(wits_host, wits_model)?),
        Some(wit_tx.clone()),
    )));
    psyche.register_typed_wit(Arc::new(MemoryWit::with_debug(
        memory.clone(),
        wit_tx.clone(),
    )));
    psyche.register_typed_wit(Arc::new(HeartWit::with_debug(
        Box::new(OllamaProvider::new(wits_host, wits_model)?),
        Arc::new(LoggingMotor),
        wit_tx.clone(),
    )));
    psyche.register_typed_wit(Arc::new(FondDuCoeurWit::new(FondDuCoeur::with_debug(
        Box::new(OllamaProvider::new(wits_host, wits_model)?),
        wit_tx.clone(),
    ))));
    psyche.set_turn_limit(usize::MAX);
    psyche
        .voice()
        .set_prompt(ContextualPrompt::new(psyche.topic_bus()));
    info!(
        %chatter_host,
        %chatter_model,
        %wits_host,
        %wits_model,
        %embeddings_host,
        %embeddings_model,
        %qdrant_url,
        %neo4j_uri,
        "created ollama psyche"
    );
    Ok(psyche)
}

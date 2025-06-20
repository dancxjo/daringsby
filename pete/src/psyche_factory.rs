use async_trait::async_trait;
use psyche::Psyche;
use psyche::ling::{Chatter, Doer, Instruction, Message, Vectorizer};
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
        wit_tx,
    )));
    psyche.set_turn_limit(usize::MAX);
    info!("created dummy psyche");
    psyche
}

/// Create a psyche backed by an Ollama server.
///
/// This uses [`OllamaProvider`](psyche::ling::OllamaProvider) for all language
/// capabilities and the no-op ear and mouth implementations.
pub fn ollama_psyche(host: &str, model: &str) -> anyhow::Result<Psyche> {
    use crate::LoggingMotor;
    use psyche::ling::OllamaProvider;
    use psyche::wits::{
        BasicMemory, Combobulator, CombobulatorWit, HeartWit, MemoryWit, Neo4jClient, QdrantClient,
        Will, WillWit,
    };

    let narrator = OllamaProvider::new(host, model)?;
    let voice = OllamaProvider::new(host, model)?;
    let vectorizer = OllamaProvider::new(host, model)?;

    let mouth = Arc::new(NoopMouth::default());
    let ear = Arc::new(NoopEar);

    let memory = Arc::new(BasicMemory {
        vectorizer: Arc::new(OllamaProvider::new(host, model)?),
        qdrant: QdrantClient::default(),
        neo4j: Arc::new(Neo4jClient::default()),
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
        Arc::new(OllamaProvider::new(host, model)?),
        wit_tx.clone(),
    )));
    psyche.register_typed_wit(Arc::new(CombobulatorWit::new(Combobulator::with_debug(
        Box::new(OllamaProvider::new(host, model)?),
        wit_tx.clone(),
    ))));
    psyche.register_typed_wit(Arc::new(WillWit::new(
        Will::with_debug(Box::new(OllamaProvider::new(host, model)?), wit_tx.clone()),
        psyche.voice(),
    )));
    psyche.register_typed_wit(Arc::new(MemoryWit::new(memory.clone())));
    psyche.register_typed_wit(Arc::new(HeartWit::new(
        Box::new(OllamaProvider::new(host, model)?),
        Arc::new(LoggingMotor),
    )));
    psyche.set_turn_limit(usize::MAX);
    info!(%host, %model, "created ollama psyche");
    Ok(psyche)
}

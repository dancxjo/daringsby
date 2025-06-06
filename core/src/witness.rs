use llm::traits::{LLMClient, LLMError};
use memory::{self, Experience, Memory, MemoryError};
use sensor::Sensation;

/// Collects raw [`Sensation`]s and pushes them into memory.
#[derive(Default)]
pub struct WitnessAgent {
    sensations: Vec<Sensation>,
}

impl WitnessAgent {
    /// Record an incoming sensation for later processing.
    pub fn ingest(&mut self, sensation: Sensation) {
        self.sensations.push(sensation);
    }

    /// Retrieve the most recent sensation, if any.
    pub fn last(&self) -> Option<&Sensation> {
        self.sensations.last()
    }

    /// Ask the LLM to explain a sensation and compute an embedding.
    pub async fn feel<C: LLMClient>(
        &mut self,
        sensation: Sensation,
        llm: &C,
    ) -> Result<Experience, LLMError> {
        self.ingest(sensation.clone());
        memory::explain_and_embed(sensation, llm).await
    }

    /// Store an [`Experience`] in the given [`Memory`] backend.
    pub async fn witness<M: Memory>(&self, exp: Experience, memory: &M) -> Result<(), MemoryError> {
        memory.store(exp).await
    }
}

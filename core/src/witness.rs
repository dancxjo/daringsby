use sensor::Sensation;
use memory::{self, Experience, Memory, MemoryError};
use llm::traits::{LLMClient, LLMError};

#[derive(Default)]
pub struct WitnessAgent {
    sensations: Vec<Sensation>,
}

impl WitnessAgent {
    pub fn ingest(&mut self, sensation: Sensation) {
        self.sensations.push(sensation);
    }

    pub fn last(&self) -> Option<&Sensation> {
        self.sensations.last()
    }

    pub async fn feel<C: LLMClient>(
        &mut self,
        sensation: Sensation,
        llm: &C,
    ) -> Result<Experience, LLMError> {
        self.ingest(sensation.clone());
        memory::explain_and_embed(sensation, llm).await
    }

    pub async fn witness<M: Memory>(
        &self,
        exp: Experience,
        memory: &M,
    ) -> Result<(), MemoryError> {
        memory.store(exp).await
    }
}

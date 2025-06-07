use crate::{VoiceAgent, VoiceOutput};
use async_trait::async_trait;

/// Streaming interface for partial and finalized prompts.
#[async_trait]
pub trait StreamingVoiceAgent: VoiceAgent {
    async fn narrate_partial(&self, context: &str) -> VoiceOutput {
        self.narrate(context).await
    }

    async fn finalize(&self, context: &str) -> VoiceOutput {
        self.narrate(context).await
    }
}

#[async_trait]
impl<T: VoiceAgent + Sync + Send> StreamingVoiceAgent for T {}

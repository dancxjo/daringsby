use llm::traits::{LLMClient, LLMError};
use llm::runner::{stream_first_sentence, client_from_env, model_from_env};
use llm::OllamaClient;

/// Evaluate Pete's emotional state as an emoji string.
pub struct MoodAgent<C: LLMClient> {
    client: C,
    model: String,
}

impl MoodAgent<OllamaClient> {
    /// Create a new mood agent using environment configuration.
    pub fn new() -> Self {
        Self {
            client: client_from_env(),
            model: model_from_env(),
        }
    }
}

impl<C: LLMClient> MoodAgent<C> {
    /// Use a custom LLM client and model name.
    pub fn with(client: C, model: impl Into<String>) -> Self {
        Self { client, model: model.into() }
    }

    /// Assess how Pete would feel about the provided context.
    pub async fn assess(&self, context: &str) -> Result<String, LLMError> {
        let prompt = format!(
            "How would the character PETE feel about this situation? Return 1-2 emojis. Situation: {context}"
        );
        log::info!("Mood prompt: {prompt}");
        let (_, resp) = stream_first_sentence(&self.client, &self.model, &prompt).await?;
        log::info!("Mood response: {resp}");
        Ok(resp.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use futures_util::stream;
    use std::pin::Pin;

    struct Mock;

    #[async_trait]
    impl LLMClient for Mock {
        async fn stream_chat(
            &self,
            _model: &str,
            _prompt: &str,
        ) -> Result<Pin<Box<dyn futures_util::Stream<Item = Result<String, LLMError>> + Send>>, LLMError> {
            Ok(Box::pin(stream::iter(vec![Ok("😀".to_string())])))
        }

        async fn embed(&self, _model: &str, _input: &str) -> Result<Vec<f32>, LLMError> {
            Ok(Vec::new())
        }
    }

    #[tokio::test]
    async fn returns_emoji() {
        let agent = MoodAgent::with(Mock, "mock");
        let emoji = agent.assess("context").await.unwrap();
        assert_eq!(emoji, "😀");
    }
}

use crate::traits::{LLMClient, LLMError};
use crate::{OllamaClient, LLMClientPool, LLMServer, LLMModel, LLMCapability};
use std::sync::Arc;
use tokio_stream::StreamExt;
use regex::Regex;

/// Stream a prompt and capture each token and the first complete sentence.
pub async fn stream_first_sentence<C: LLMClient>(
    client: &C,
    model: &str,
    prompt: &str,
) -> Result<(Vec<String>, String), LLMError> {
    let mut stream = client.stream_chat(model, prompt).await?;
    let re = Regex::new(r"[.!?]\s").unwrap();
    let mut tokens = Vec::new();
    let mut buffer = String::new();
    let mut sentence = None;
    while let Some(chunk) = stream.next().await {
        let token = chunk?;
        buffer.push_str(&token);
        tokens.push(token);
        if sentence.is_none() {
            if let Some(m) = re.find(&buffer) {
                sentence = Some(buffer[..m.end()].to_string());
            }
        }
    }
    let sentence = sentence.unwrap_or_else(|| buffer.clone());
    Ok((tokens, sentence))
}

/// Create an [`OllamaClient`] using the `OLLAMA_URL` environment variable.
pub fn client_from_env() -> OllamaClient {
    let url = std::env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".into());
    OllamaClient::new(&url)
}

/// Read the chat model name from the `OLLAMA_MODEL` environment variable.
pub fn model_from_env() -> String {
    std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "gemma3:27b".into())
}

/// Create a [`LinguisticScheduler`] from the `OLLAMA_URLS` environment variable.
/// The variable should contain a comma separated list of base URLs. If not set,
/// `OLLAMA_URL` is used instead.
pub fn scheduler_from_env() -> LLMClientPool {
    let urls = std::env::var("OLLAMA_URLS")
        .or_else(|_| std::env::var("OLLAMA_URL"))
        .unwrap_or_else(|_| "http://localhost:11434".into());
    let model = model_from_env();
    let mut pool = LLMClientPool::new();
    for url in urls.split(',') {
        let client = Arc::new(OllamaClient::new(url.trim()));
        let server = LLMServer::new(client).with_model(LLMModel::new(
            model.clone(),
            vec![LLMCapability::Chat],
        ));
        pool.add_server(server);
    }
    pool
}

/// Convenience helper to stream a prompt using environment configuration.
pub async fn run_from_env(prompt: &str) -> Result<(Vec<String>, String), LLMError> {
    let client = client_from_env();
    let model = model_from_env();
    stream_first_sentence(&client, &model, prompt).await
}

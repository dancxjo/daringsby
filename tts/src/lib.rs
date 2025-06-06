//! Convert LLM output into audio using Coqui TTS.

use emojito::find_emoji;
use llm::{LLMClient, LLMError, OllamaClient};
use regex::Regex;
use reqwest::Client;
use std::env;
use thiserror::Error;
use tokio_stream::StreamExt;

#[derive(Debug, Error)]
pub enum TTSError {
    #[error(transparent)]
    LLM(#[from] LLMError),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
}

/// Convenience result type used throughout this crate.
pub type Result<T> = std::result::Result<T, TTSError>;

fn strip_emojis(input: &str) -> (String, Vec<String>) {
    let found = find_emoji(input);
    let mut cleaned = input.to_string();
    for e in &found {
        cleaned = cleaned.replace(e.glyph, "");
    }
    let emojis = found.iter().map(|e| e.glyph.to_string()).collect();
    (cleaned, emojis)
}

/// Stream text from the LLM, capture the first sentence and synthesize speech.
pub async fn speak_from_llm(prompt: &str) -> Result<Vec<u8>> {
    let ollama_url = env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".into());
    let coqui_url = env::var("COQUI_URL").unwrap_or_else(|_| "http://localhost:5002".into());
    let speaker = env::var("SPEAKER").unwrap_or_else(|_| "default".into());

    let llm = OllamaClient::new(&ollama_url);
    let mut stream = llm.stream_chat("gemma3:27b", prompt).await?;
    let re = Regex::new(r"[.!?]\s").unwrap();
    let mut buffer = String::new();
    let mut sentence = None;

    while let Some(chunk) = stream.next().await {
        let piece = chunk?;
        buffer.push_str(&piece);
        if let Some(mat) = re.find(&buffer) {
            sentence = Some(buffer[..mat.end()].to_string());
            break;
        }
    }

    let sentence = sentence.unwrap_or(buffer);
    let (clean, emojis) = strip_emojis(&sentence);
    let emotion = if emojis.is_empty() {
        None
    } else {
        Some(emojis.join(""))
    };

    #[derive(serde::Serialize)]
    struct TtsRequest<'a> {
        text: &'a str,
        speaker_id: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        emotion: Option<String>,
    }

    let payload = TtsRequest {
        text: clean.trim(),
        speaker_id: &speaker,
        emotion,
    };

    let client = Client::new();
    let res = client
        .post(format!("{}/api/tts", coqui_url))
        .json(&payload)
        .send()
        .await?;
    let bytes = res.bytes().await?;
    Ok(bytes.to_vec())
}

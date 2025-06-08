use voice::{model::MockModelClient, ChatVoice, VoiceAgent};

#[tokio::test]
async fn voice_updates_conversation() {
    let llm = MockModelClient::new(vec!["ok".into()], vec![]);
    let voice = ChatVoice::new(llm, "mock", 3);
    voice.receive_user("hi");
    let out = voice.narrate("test").await;
    assert_eq!(out.think.content, "");
    assert_eq!(out.say.unwrap().content, "ok");
}

#[tokio::test]
async fn parses_think_silently() {
    let llm = MockModelClient::new(
        vec!["hi <think-silently>secret</think-silently>".into()],
        vec![],
    );
    let voice = ChatVoice::new(llm, "mock", 3);
    voice.receive_user("yo");
    let out = voice.narrate("ctx").await;
    assert_eq!(out.think.content, "secret");
    assert_eq!(out.say.unwrap().content, "hi");
}

use async_trait::async_trait;
use futures_core::Stream;
use std::{
    pin::Pin,
    sync::{Arc, Mutex},
};
use tokio_stream::iter;
use voice::model::{ModelClient, ModelError};

struct RecordingClient {
    prompt: Arc<Mutex<Option<String>>>,
}

impl RecordingClient {
    fn new(prompt: Arc<Mutex<Option<String>>>) -> Self {
        Self { prompt }
    }
}

#[async_trait]
impl ModelClient for RecordingClient {
    async fn stream_chat(
        &self,
        _model: &str,
        prompt: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, ModelError>> + Send>>, ModelError> {
        *self.prompt.lock().unwrap() = Some(prompt.to_string());
        Ok(Box::pin(iter(vec![Ok("ok".to_string())])))
    }

    async fn embed(&self, _model: &str, _input: &str) -> Result<Vec<f32>, ModelError> {
        Ok(vec![])
    }
}

#[tokio::test]
async fn prompt_mentions_terse_rule() {
    let captured = Arc::new(Mutex::new(None));
    let llm = RecordingClient::new(captured.clone());
    let voice = ChatVoice::new(llm, "mock", 1);
    let _ = voice.narrate("context").await;
    let prompt = captured.lock().unwrap().clone().unwrap();
    assert!(prompt.contains("Keep your responses brief"));
}

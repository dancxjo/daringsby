#![cfg(feature = "tts")]
use futures::stream;
use pete::{Tts, TtsMouth, TtsStream};
use psyche::Event;
use psyche::traits::Mouth;
use std::sync::{Arc, atomic::AtomicBool};
use tokio::sync::broadcast;

struct DummyTts;

#[async_trait::async_trait]
impl Tts for DummyTts {
    async fn stream_wav(&self, _text: &str) -> anyhow::Result<TtsStream> {
        Ok(Box::pin(stream::once(async { Ok(vec![0u8; 4]) })))
    }
}

#[tokio::test]
async fn emits_audio_events() {
    let (tx, mut rx) = broadcast::channel(8);
    let mouth = TtsMouth::new(
        tx.clone(),
        Arc::new(AtomicBool::new(false)),
        Arc::new(DummyTts),
    );
    mouth.speak("Hello world.").await;
    match rx.recv().await {
        Ok(Event::Speech {
            text,
            audio: Some(a),
        }) => {
            assert_eq!(text, "Hello world.");
            assert!(!a.is_empty());
        }
        other => panic!("unexpected event: {:?}", other),
    }
}

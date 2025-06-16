#![cfg(feature = "tts")]
use pete::{Tts, TtsMouth};
use psyche::Event;
use psyche::Mouth;
use std::sync::{Arc, atomic::AtomicBool};
use tokio::sync::broadcast;

struct DummyTts;

impl Tts for DummyTts {
    fn to_wav(&self, _text: &str) -> anyhow::Result<Vec<u8>> {
        Ok(vec![0u8; 4])
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
    if let Ok(Event::SpeechAudio(_)) = rx.recv().await {
        return;
    }
    panic!("no audio event");
}

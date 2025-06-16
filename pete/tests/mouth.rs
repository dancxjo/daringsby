use pete::ChannelMouth;
use psyche::{Event, Mouth};
use std::sync::{Arc, atomic::AtomicBool};
use tokio::sync::broadcast;

#[tokio::test]
async fn sends_sentence_by_sentence() {
    let (tx, mut rx) = broadcast::channel(8);
    let mouth = ChannelMouth::new(tx.clone(), Arc::new(AtomicBool::new(false)));
    mouth.speak("Hello world. How are you?").await;
    assert_eq!(
        rx.recv().await.unwrap(),
        Event::IntentionToSay("Hello world.".into())
    );
    assert_eq!(
        rx.recv().await.unwrap(),
        Event::IntentionToSay("How are you?".into())
    );
}

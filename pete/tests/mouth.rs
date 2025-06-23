use pete::{ChannelMouth, EventBus};
use psyche::Event;
use psyche::traits::Mouth;
use std::sync::{Arc, atomic::AtomicBool};

#[tokio::test]
async fn sends_sentence_by_sentence() {
    let (bus, _) = EventBus::new();
    let bus = Arc::new(bus);
    let mut rx = bus.subscribe_events();
    let mouth = ChannelMouth::new(bus.clone(), Arc::new(AtomicBool::new(false)));
    mouth.speak("Hello world. How are you?").await;
    assert_eq!(
        rx.recv().await.unwrap(),
        Event::Speech {
            text: "Hello world.".into(),
            audio: None
        }
    );
    assert_eq!(
        rx.recv().await.unwrap(),
        Event::Speech {
            text: "How are you?".into(),
            audio: None
        }
    );
}

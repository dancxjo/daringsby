use net::stream_bus::{StreamBus, StreamEvent, ConversationRole};

#[tokio::test]
async fn broadcast_roundtrip() {
    let bus = StreamBus::new(4);
    let mut sub = bus.subscribe();
    bus.send(StreamEvent::AsrFinal {
        transcript: "hi".into(),
    })
    .unwrap();
    let evt = sub.recv().await.unwrap();
    assert_eq!(
        evt,
        StreamEvent::AsrFinal {
            transcript: "hi".into()
        }
    );
}

#[tokio::test]
async fn conversation_update_roundtrip() {
    let bus = StreamBus::new(4);
    let mut sub = bus.subscribe();
    bus.send(StreamEvent::ConversationUpdate {
        role: ConversationRole::Assistant,
        content: "hi".into(),
    })
    .unwrap();
    let evt = sub.recv().await.unwrap();
    assert_eq!(
        evt,
        StreamEvent::ConversationUpdate {
            role: ConversationRole::Assistant,
            content: "hi".into()
        }
    );
}

use async_trait::async_trait;
use axum::{Router, routing::get, serve};
use futures::{SinkExt, StreamExt};
use pete::{Body, EventBus, EyeSensor, GeoSensor, dummy_psyche, ws_handler};
use psyche::traits::Ear;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::{Mutex, mpsc};

struct RecordingEar {
    heard: mpsc::UnboundedSender<String>,
    self_heard: Arc<AtomicUsize>,
}

#[async_trait]
impl Ear for RecordingEar {
    async fn hear_self_say(&self, _text: &str) {
        self.self_heard.fetch_add(1, Ordering::SeqCst);
    }

    async fn hear_user_say(&self, text: &str) {
        let _ = self.heard.send(text.to_string());
    }
}

#[tokio::test]
async fn websocket_text_is_reported_to_ear() {
    let psyche = dummy_psyche();
    let conversation = psyche.conversation();
    let (heard_tx, mut heard_rx) = mpsc::unbounded_channel();
    let ear = Arc::new(RecordingEar {
        heard: heard_tx,
        self_heard: Arc::new(AtomicUsize::new(0)),
    });
    let eye = Arc::new(EyeSensor::new(psyche.input_sender()));
    let geo = Arc::new(GeoSensor::new(psyche.input_sender()));
    let (bus, _user_rx) = EventBus::new();
    let bus = Arc::new(bus);
    let debug = psyche.debug_handle();
    let state = Body {
        asr: None,
        bus,
        ear,
        eye,
        geo,
        conversation,
        connections: Arc::new(AtomicUsize::new(0)),
        system_prompt: Arc::new(Mutex::new(psyche.system_prompt())),
        psyche_debug: debug,
    };
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        serve(listener, app.into_make_service()).await.unwrap();
    });

    let (mut socket, _) = tokio_tungstenite::connect_async(format!("ws://{}/ws", addr))
        .await
        .unwrap();
    let _ = socket.next().await.unwrap().unwrap();
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::json!({
                "type": "Text",
                "text": "hello pete"
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();

    let heard = tokio::time::timeout(std::time::Duration::from_secs(1), heard_rx.recv())
        .await
        .expect("timed out waiting for websocket text to reach ear")
        .expect("ear channel closed");
    assert_eq!(heard, "hello pete");

    server.abort();
}

use axum::{Router, routing::get, serve};
use futures::StreamExt;
use pete::{AppState, ChannelEar, EyeSensor, dummy_psyche, ws_handler};
use psyche::Event;
use psyche::Sensor;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize},
};
use tokio::sync::{broadcast, mpsc};

#[tokio::test]
async fn websocket_forwards_audio() {
    let mut psyche = dummy_psyche();
    let conversation = psyche.conversation();
    let ear = Arc::new(ChannelEar::new(
        psyche.input_sender(),
        conversation.clone(),
        Arc::new(AtomicBool::new(false)),
    ));
    let eye = Arc::new(EyeSensor::new(psyche.input_sender()));
    psyche.add_sense(eye.description());
    let (event_tx, _) = broadcast::channel(8);
    let (wit_tx, _) = broadcast::channel(8);
    let (log_tx, _) = broadcast::channel(8);
    let (user_tx, _user_rx) = mpsc::unbounded_channel();
    let state = AppState {
        user_input: user_tx,
        events: Arc::new(event_tx.subscribe()),
        logs: Arc::new(log_tx.subscribe()),
        wits: Arc::new(wit_tx.subscribe()),
        ear,
        eye,
        conversation,
        connections: Arc::new(AtomicUsize::new(0)),
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
    event_tx
        .send(Event::SpeechAudio("UklGRg==".into()))
        .unwrap();
    let msg = socket.next().await.unwrap().unwrap();
    let value: serde_json::Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
    assert_eq!(value["type"], "pete-audio");
    assert_eq!(value["audio"], "UklGRg==");
    server.abort();
}

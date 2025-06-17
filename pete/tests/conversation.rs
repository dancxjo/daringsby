use axum::{body, extract::State, response::IntoResponse};
use pete::{AppState, ChannelEar, conversation_log, dummy_psyche};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize},
};
use tokio::sync::{broadcast, mpsc};

#[tokio::test]
async fn returns_log_json() {
    let psyche = dummy_psyche();
    let conversation = psyche.conversation();
    conversation.lock().await.add_user("hi".into());
    let ear = Arc::new(ChannelEar::new(
        psyche.input_sender(),
        conversation.clone(),
        Arc::new(AtomicBool::new(false)),
    ));
    let (event_tx, _) = broadcast::channel(8);
    let (log_tx, _) = broadcast::channel(8);
    let (user_tx, _user_rx) = mpsc::unbounded_channel();
    let state = AppState {
        user_input: user_tx,
        events: Arc::new(event_tx.subscribe()),
        logs: Arc::new(log_tx.subscribe()),
        ear,
        conversation,
        connections: Arc::new(AtomicUsize::new(1)),
    };
    let resp = conversation_log(State(state)).await.into_response();
    let body = body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let msgs: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(msgs[0]["role"], "user");
    assert_eq!(msgs[0]["content"], "hi");
}

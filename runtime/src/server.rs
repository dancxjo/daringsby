use axum::{extract::{ws::{WebSocket, WebSocketUpgrade, Message}, State}, response::{Html, IntoResponse}, routing::get, Router};
use serde::Deserialize;
use tokio::sync::{mpsc::Sender, Mutex as AsyncMutex};
use sensor::Sensation;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use std::sync::Arc;
use crate::logger::SimpleLogger;

#[derive(Clone)]
pub struct AppState {
    pub tx: Sender<Sensation>,
    pub mood: Arc<AsyncMutex<String>>,
    pub logs: Arc<SimpleLogger>,
}

pub fn router(tx: Sender<Sensation>, mood: Arc<AsyncMutex<String>>, logs: Arc<SimpleLogger>) -> Router {
    Router::new()
        .route("/see", get(index))
        .route("/see/ws", get(ws_handler))
        .route("/face", get(face))
        .route("/face/emoji", get(face_emoji))
        .route("/logs", get(show_logs))
        .with_state(AppState { tx, mood, logs })
}

async fn index() -> Html<&'static str> {
    Html(include_str!("see.html"))
}

async fn face() -> Html<&'static str> {
    Html(include_str!("face.html"))
}

async fn face_emoji(State(state): State<AppState>) -> String {
    state.mood.lock().await.clone()
}

async fn show_logs(State(state): State<AppState>) -> Html<String> {
    Html(format!("<pre>{}</pre>", state.logs.dump()))
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

#[derive(Deserialize)]
struct Frame { base64: String }

async fn handle_ws(mut socket: WebSocket, state: AppState) {
    while let Some(Ok(Message::Text(text))) = socket.recv().await {
        if let Ok(frame) = serde_json::from_str::<Frame>(&text) {
            if let Ok(bytes) = BASE64.decode(frame.base64) {
                let _ = state.tx.send(Sensation::saw(bytes)).await;
            }
        }
    }
}


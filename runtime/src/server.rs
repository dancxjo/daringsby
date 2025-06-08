use crate::logger::SimpleLogger;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use sensor::Sensation;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::{mpsc::Sender, Mutex as AsyncMutex};

#[derive(Clone)]
pub struct AppState {
    pub tx: Sender<Sensation>,
    pub mood: Arc<AsyncMutex<String>>,
    pub logs: Arc<SimpleLogger>,
}

pub fn router(
    tx: Sender<Sensation>,
    mood: Arc<AsyncMutex<String>>,
    logs: Arc<SimpleLogger>,
) -> Router {
    Router::new()
        .route("/", get(home))
        .route("/see", get(see))
        .route("/ws", get(ws_handler))
        .route("/see/ws", get(ws_frames))
        .route("/face", get(face))
        .route("/face/emoji", get(face_emoji))
        .route("/logs", get(show_logs))
        .with_state(AppState { tx, mood, logs })
}

async fn home() -> Html<&'static str> {
    Html(include_str!("index.html"))
}

async fn see() -> Html<&'static str> {
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
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Input {
    Text { value: String },
    Audio { base64: String },
    Geo { lat: f64, lon: f64 },
    Frame { base64: String },
}

#[derive(Deserialize)]
struct Frame {
    base64: String,
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    while let Some(Ok(Message::Text(text))) = socket.recv().await {
        if let Ok(input) = serde_json::from_str::<Input>(&text) {
            match input {
                Input::Text { value } => {
                    let _ = state.tx.send(Sensation::new("keyboard", Some(value))).await;
                }
                Input::Audio { base64 } => {
                    if let Ok(bytes) = BASE64.decode(base64) {
                        // placeholder for ASR processing
                        let _ = state
                            .tx
                            .send(Sensation::new(
                                "audio",
                                Some(format!("{} bytes", bytes.len())),
                            ))
                            .await;
                    }
                }
                Input::Geo { lat, lon } => {
                    let _ = state
                        .tx
                        .send(Sensation::new("geo", Some(format!("{lat},{lon}"))))
                        .await;
                }
                Input::Frame { base64 } => {
                    if let Ok(bytes) = BASE64.decode(base64) {
                        let _ = state.tx.send(Sensation::saw(bytes)).await;
                    }
                }
            }
        }
    }
}

async fn ws_frames(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_frames(socket, state))
}

async fn handle_frames(mut socket: WebSocket, state: AppState) {
    while let Some(Ok(Message::Text(text))) = socket.recv().await {
        if let Ok(frame) = serde_json::from_str::<Frame>(&text) {
            if let Ok(bytes) = BASE64.decode(frame.base64) {
                let _ = state.tx.send(Sensation::saw(bytes)).await;
            }
        }
    }
}

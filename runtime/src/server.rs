use axum::{extract::{ws::{WebSocket, WebSocketUpgrade, Message}, State}, response::{Html, IntoResponse}, routing::get, Router};
use serde::Deserialize;
use tokio::sync::mpsc::Sender;
use sensor::Sensation;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;

#[derive(Clone)]
pub struct AppState {
    pub tx: Sender<Sensation>,
}

pub fn router(tx: Sender<Sensation>) -> Router {
    Router::new()
        .route("/see", get(index))
        .route("/see/ws", get(ws_handler))
        .with_state(AppState { tx })
}

async fn index() -> Html<&'static str> {
    Html(include_str!("see.html"))
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


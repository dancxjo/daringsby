use axum::{extract::ws::{WebSocket, WebSocketUpgrade, Message}, routing::get, Router};
use axum::response::IntoResponse;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Deserialize)]
#[serde(tag="sensor_type", rename_all="lowercase")]
pub enum SensorInput {
    Geolocation { lat: f64, lon: f64 },
    Audio { transcript: String },
    Image { base64: String },
    Text { value: String },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Sensation {
    pub how: String,
    pub when: DateTime<Utc>,
}

pub async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    while let Some(Ok(msg)) = socket.recv().await {
        if let Message::Text(text) = msg {
            if let Ok(input) = serde_json::from_str::<SensorInput>(&text) {
                let how = match input {
                    SensorInput::Geolocation { lat, lon } => format!("geo {lat},{lon}"),
                    SensorInput::Audio { transcript } => format!("audio {transcript}"),
                    SensorInput::Image { .. } => "image".to_string(),
                    SensorInput::Text { value } => value,
                };
                let sensation = Sensation { how, when: Utc::now() };
                let _ = socket
                    .send(Message::Text(serde_json::to_string(&sensation).unwrap()))
                    .await;
                info!(?sensation, "received");
            }
        }
    }
}

async fn dev_panel() -> impl IntoResponse {
    const HTML: &str = include_str!("devpanel.html");
    HTML
}

pub fn router() -> Router {
    Router::new()
        .route("/ws", get(ws_handler))
        .route("/devpanel", get(dev_panel))
}

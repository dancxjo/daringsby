use axum::{
    Router,
    extract::{
        State,
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, get_service},
};
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::{broadcast, mpsc};
use tower_http::services::ServeDir;
use tracing::{debug, error, info};

use psyche::{Ear, Event, ImageData, Sensor, ling::Role};

/// State shared across HTTP handlers and WebSocket tasks.
#[derive(Clone)]
pub struct AppState {
    pub user_input: mpsc::UnboundedSender<String>,
    pub events: Arc<broadcast::Receiver<Event>>,
    pub logs: Arc<broadcast::Receiver<String>>,
    pub wits: Arc<broadcast::Receiver<psyche::WitReport>>,
    pub ear: Arc<dyn Ear>,
    pub eye: Arc<dyn Sensor<ImageData>>,
    pub conversation: Arc<tokio::sync::Mutex<psyche::Conversation>>,
    pub connections: Arc<AtomicUsize>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum WsRequest {
    Text { data: String },
    Echo { data: String },
    See { data: String, at: Option<String> },
    Hear { data: String, at: Option<String> },
    Geolocate { data: GeoLoc, at: Option<String> },
    Sense { data: serde_json::Value },
}

#[derive(Deserialize)]
pub struct GeoLoc {
    pub longitude: f64,
    pub latitude: f64,
}

#[derive(Serialize)]
#[serde(tag = "type", content = "data")]
enum WsResponse {
    #[serde(rename = "say")]
    Say {
        words: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        audio: Option<String>,
    },
    #[serde(rename = "emote")]
    Emote(String),
    #[serde(rename = "think")]
    Think(String),
    #[serde(rename = "heard")]
    Heard(String),
}

pub async fn index() -> Html<&'static str> {
    info!("index requested");
    Html("WebSocket server is running. Connect your client to /ws")
}

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    info!("websocket upgrade initiated");
    ws.on_upgrade(move |socket| async move { handle_socket(socket, state).await })
}

pub async fn log_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    info!("log websocket upgrade initiated");
    ws.on_upgrade(move |socket| async move { handle_log_socket(socket, state).await })
}

pub async fn wit_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    info!("wit websocket upgrade initiated");
    ws.on_upgrade(move |socket| async move { handle_wit_socket(socket, state).await })
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    info!("websocket connected");
    state.connections.fetch_add(1, Ordering::SeqCst);
    let mut events = state.events.resubscribe();
    loop {
        tokio::select! {
            evt = events.recv() => {
                match evt {
                    Ok(Event::Speech { text, audio }) => {
                        let payload = serde_json::to_string(&WsResponse::Say { words: text, audio }).unwrap();
                        if socket.send(WsMessage::Text(payload.into())).await.is_err() {
                            error!("failed sending speech");
                            break;
                        }
                    }
                    Ok(Event::StreamChunk(_)) => {},
                    Ok(Event::EmotionChanged(emo)) => {
                        let payload = serde_json::to_string(&WsResponse::Emote(emo)).unwrap();
                        if socket.send(WsMessage::Text(payload.into())).await.is_err() {
                            error!("failed sending emotion");
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(WsMessage::Text(text))) => {
                        if let Ok(req) = serde_json::from_str::<WsRequest>(&text) {
                            match req {
                                WsRequest::Text { data: message } => {
                                    debug!("user message: {}", message);
                                    let _ = state.user_input.send(message.clone());
                                    let payload = serde_json::to_string(&WsResponse::Heard(message)).unwrap();
                                    if socket.send(WsMessage::Text(payload.into())).await.is_err() {
                                        error!("failed sending heard ack");
                                        break;
                                    }
                                }
                                WsRequest::Echo { data: text } => {
                                    debug!("played ack: {}", text);
                                    state.ear.hear_self_say(&text).await;
                                }
                                WsRequest::See { data, .. } => {
                                    if let Some((mime, base64)) = parse_data_url(&data) {
                                        debug!("image received");
                                        state.eye.sense(ImageData { mime, base64 }).await;
                                    }
                                }
                                WsRequest::Hear { .. } => {
                                    debug!("audio fragment received");
                                }
                                WsRequest::Geolocate { .. } => {
                                    debug!("geolocation received");
                                }
                                WsRequest::Sense { .. } => {
                                    debug!("sense event received");
                                }
                            }
                        }
                    }
                    Some(Ok(WsMessage::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
    state.connections.fetch_sub(1, Ordering::SeqCst);
    info!("websocket disconnected");
}

async fn handle_log_socket(mut socket: WebSocket, state: AppState) {
    info!("log websocket connected");
    let mut logs = state.logs.resubscribe();
    while let Ok(line) = logs.recv().await {
        if socket.send(WsMessage::Text(line.into())).await.is_err() {
            break;
        }
    }
    info!("log websocket disconnected");
}

async fn handle_wit_socket(mut socket: WebSocket, state: AppState) {
    info!("wit websocket connected");
    let mut rx = state.wits.resubscribe();
    while let Ok(report) = rx.recv().await {
        let msg = serde_json::to_string(&WsResponse::Think(report.output.clone())).unwrap();
        if socket.send(WsMessage::Text(msg.into())).await.is_err() {
            break;
        }
    }
    info!("wit websocket disconnected");
}

pub async fn conversation_log(State(state): State<AppState>) -> impl IntoResponse {
    let conv = state.conversation.lock().await;
    #[derive(Serialize)]
    struct Entry {
        role: String,
        content: String,
    }
    let entries: Vec<Entry> = conv
        .all()
        .iter()
        .map(|m| Entry {
            role: match m.role {
                Role::User => "user".to_string(),
                Role::Assistant => "assistant".to_string(),
            },
            content: m.content.clone(),
        })
        .collect();
    axum::Json(entries)
}

fn parse_data_url(url: &str) -> Option<(String, String)> {
    let (prefix, data) = url.split_once(',')?;
    let mime = prefix
        .trim_start_matches("data:")
        .trim_end_matches(";base64");
    Some((mime.to_string(), data.to_string()))
}

pub async fn listen_user_input(mut rx: mpsc::UnboundedReceiver<String>, ear: Arc<dyn Ear>) {
    while let Some(msg) = rx.recv().await {
        debug!("forwarding user input: {}", msg);
        ear.hear_user_say(&msg).await;
    }
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/ws", get(ws_handler))
        .route("/log", get(log_ws_handler))
        .route("/debug", get(wit_ws_handler))
        .route("/conversation", get(conversation_log))
        .fallback_service(
            get_service(ServeDir::new("../frontend/dist"))
                .handle_error(|_| async { StatusCode::INTERNAL_SERVER_ERROR }),
        )
        .with_state(state)
}

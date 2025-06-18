use axum::{
    Router,
    extract::{
        State,
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
    },
    response::{Html, IntoResponse},
    routing::get,
};
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, error, info};

use psyche::{Ear, Event, ImageData, Sensor, ling::Role};

/// State shared across HTTP handlers and WebSocket tasks.
#[derive(Clone)]
pub struct AppState {
    pub user_input: mpsc::UnboundedSender<String>,
    pub events: Arc<broadcast::Receiver<Event>>,
    pub logs: Arc<broadcast::Receiver<String>>,
    pub ear: Arc<dyn Ear>,
    pub eye: Arc<dyn Sensor<ImageData>>,
    pub conversation: Arc<tokio::sync::Mutex<psyche::Conversation>>,
    pub connections: Arc<AtomicUsize>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum WsRequest {
    /// A message from the user.
    User {
        message: String,
        #[allow(dead_code)]
        name: Option<String>,
    },
    /// Confirmation that a line was displayed to the user.
    Displayed { text: String },
    /// Confirmation that audio for the line was played.
    Played { text: String },
    /// A base64-encoded image snapshot.
    Image { mime: String, base64: String },
}

#[derive(Serialize)]
struct WsResponse<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    audio: Option<String>,
}

/// Serve the chat page.
pub async fn index() -> Html<&'static str> {
    info!("serving index page");
    Html(include_str!("../../index.html"))
}

/// Upgrade the request to a WebSocket connection and forward events.
pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    info!("websocket upgrade initiated");
    ws.on_upgrade(move |socket| async move { handle_socket(socket, state).await })
}

/// Upgrade to a WebSocket streaming log output.
pub async fn log_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    info!("log websocket upgrade initiated");
    ws.on_upgrade(move |socket| async move { handle_log_socket(socket, state).await })
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    info!("websocket connected");
    state.connections.fetch_add(1, Ordering::SeqCst);
    let mut events = state.events.resubscribe();
    loop {
        tokio::select! {
            evt = events.recv() => {
                match evt {
                    Ok(Event::StreamChunk(chunk)) => {
                        let payload = serde_json::to_string(&WsResponse { kind: "pete-says", text: Some(chunk), audio: None }).unwrap();
                        if socket.send(WsMessage::Text(payload.into())).await.is_err() {
                            error!("failed sending chunk");
                            break;
                        }
                    }
                    Ok(Event::IntentionToSay(_)) => {
                        // Intention events are used internally to trigger audio
                        // playback. The user already received the text via
                        // `StreamChunk` messages, so skip forwarding this final
                        // repetition to avoid duplicate lines in the chat log.
                        debug!("ws skipping IntentionToSay");
                    }
                    Ok(Event::SpeechAudio(data)) => {
                        let payload = serde_json::to_string(&WsResponse { kind: "pete-audio", text: None, audio: Some(data) }).unwrap();
                        debug!("ws dispatch audio chunk");
                        if socket.send(WsMessage::Text(payload.into())).await.is_err() {
                            error!("failed sending audio");
                            break;
                        }
                    }
                    Ok(Event::EmotionChanged(emo)) => {
                        let payload = serde_json::to_string(&WsResponse { kind: "pete-emotion", text: Some(emo), audio: None }).unwrap();
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
                                WsRequest::User { message, .. } => {
                                    debug!("user message: {}", message);
                                    let _ = state.user_input.send(message);
                                }
                                WsRequest::Displayed { text } => {
                                    debug!("displayed ack: {}", text);
                                    state.ear.hear_self_say(&text).await;
                                }
                                WsRequest::Played { text } => {
                                    debug!("played ack: {}", text);
                                    state.ear.hear_self_say(&text).await;
                                }
                                WsRequest::Image { mime, base64 } => {
                                    debug!("image received");
                                    state
                                        .eye
                                        .sense(ImageData { mime, base64 })
                                        .await;
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

/// Return the raw conversation log as JSON.
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

/// Listen for user input messages and record them in the conversation.
///
/// Each received message is forwarded to the running [`Psyche`] via
/// `Sensation::HeardUserVoice` and appended to the shared conversation log.
///
/// ```no_run
/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
/// # use pete::{dummy_psyche, listen_user_input, ChannelEar};
/// # use tokio::sync::mpsc;
/// let mut psyche = dummy_psyche();
/// let conv = psyche.conversation();
/// let speaking = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
/// let ear = std::sync::Arc::new(ChannelEar::new(psyche.input_sender(), conv.clone(), speaking));
/// let (tx, rx) = mpsc::unbounded_channel();
/// tokio::spawn(listen_user_input(rx, ear));
/// # tx.send("hi".into()).unwrap();
/// # });
/// ```
pub async fn listen_user_input(mut rx: mpsc::UnboundedReceiver<String>, ear: Arc<dyn Ear>) {
    while let Some(msg) = rx.recv().await {
        debug!("forwarding user input: {}", msg);
        ear.hear_user_say(&msg).await;
    }
}

/// Build the application router with the provided state.
pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/ws", get(ws_handler))
        .route("/log", get(log_ws_handler))
        .route("/conversation", get(conversation_log))
        .with_state(state)
}

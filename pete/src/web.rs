use axum::{
    Json, Router,
    extract::{
        Path, State,
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, get_service},
};
#[cfg(feature = "asr")]
use base64::Engine;
#[cfg(feature = "asr")]
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::{ConversationEntry as ConvEntry, WsPayload};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::{broadcast, mpsc};
use tower_http::services::ServeDir;
#[cfg(feature = "asr")]
use tracing::warn;
use tracing::{debug, error, info, trace};

use crate::EventBus;
use lingproc::Role;
use psyche::{BrowserMotion, Ear, Event, GeoLoc, ImageData, Sensor};

/// PETE's interface to the world — his `Body`.
///
/// The `Body` struct wires together PETE's sensory inputs, expressive outputs,
/// and shared state for the web interface. It is passed into HTTP and WebSocket
/// routes to serve as PETE’s physical and social connection to the outside world.
///
/// ## Responsibilities
/// - 🧠 Connects the web server to the running [`Psyche`] instance
/// - 👁 Streams image input via [`Sensor<ImageData>`]
/// - 📍 Receives geolocation input via [`Sensor<GeoLoc>`]
/// - 📱 Receives browser motion input via [`Sensor<BrowserMotion>`]
/// - 👂 Lets Pete “hear” the user via the [`Ear`] trait
/// - 🗣 Shares and modifies the current [`Conversation`] log
/// - 🪞 Exposes introspection via [`DebugHandle`]
/// - 🔌 Tracks the number of active WebSocket connections
///
/// This struct represents Pete’s *body* — his live connection to the world of
/// sensation and interaction.
#[derive(Clone)]
pub struct Body {
    #[cfg(feature = "asr")]
    pub asr: Option<Arc<crate::AsrService>>,
    pub bus: Arc<EventBus>,
    pub ear: Arc<dyn Ear>,
    pub eye: Arc<dyn Sensor<ImageData>>,
    pub geo: Arc<dyn Sensor<GeoLoc>>,
    pub motion: Arc<dyn Sensor<BrowserMotion>>,
    pub conversation: Arc<tokio::sync::Mutex<psyche::Conversation>>,
    pub connections: Arc<AtomicUsize>,
    pub system_prompt: Arc<tokio::sync::Mutex<String>>,
    pub psyche_debug: psyche::DebugHandle,
}

pub type WsRequest = WsPayload;
pub type WsResponse = WsPayload;

pub async fn index() -> Html<&'static str> {
    debug!("index requested");
    Html(include_str!("../../frontend/dist/index.html"))
}

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Body>) -> impl IntoResponse {
    debug!("websocket upgrade initiated");
    ws.on_upgrade(move |socket| async move { handle_socket(socket, state).await })
}

pub async fn log_ws_handler(ws: WebSocketUpgrade, State(state): State<Body>) -> impl IntoResponse {
    debug!("log websocket upgrade initiated");
    ws.on_upgrade(move |socket| async move { handle_log_socket(socket, state).await })
}

pub async fn wit_ws_handler(ws: WebSocketUpgrade, State(state): State<Body>) -> impl IntoResponse {
    debug!("wit websocket upgrade initiated");
    ws.on_upgrade(move |socket| async move { handle_wit_socket(socket, state).await })
}

async fn handle_socket(mut socket: WebSocket, state: Body) {
    info!("websocket connected");
    state.connections.fetch_add(1, Ordering::SeqCst);
    let mut events = state.bus.subscribe_events();
    let mut wits = state.bus.subscribe_wits();
    let (asr_text_tx, mut asr_text_rx) = mpsc::channel::<(String, DateTime<Utc>)>(64);
    let mut asr_open = false;
    #[cfg(feature = "asr")]
    let asr_pcm_tx = if let Some(asr) = state.asr.clone() {
        let (pcm_tx, mut transcript_rx) = asr.spawn_connection();
        let text_tx = asr_text_tx.clone();
        asr_open = true;
        tokio::spawn(async move {
            while let Some(transcript) = transcript_rx.recv().await {
                let text = transcript.text.trim().to_string();
                if !text.is_empty() && text_tx.send((text, transcript.occurred_at)).await.is_err() {
                    break;
                }
            }
        });
        Some(pcm_tx)
    } else {
        None
    };
    drop(asr_text_tx);
    let prompt = state.system_prompt.lock().await.clone();
    let _ = socket
        .send(WsMessage::Text(
            serde_json::to_string(&WsResponse::SystemPrompt(prompt))
                .unwrap()
                .into(),
        ))
        .await;
    let conv = state.conversation.lock().await;
    for entry in conv.all_with_timestamps() {
        let item = ConvEntry {
            role: match entry.message.role {
                Role::User => "user".into(),
                Role::Assistant => "assistant".into(),
            },
            content: entry.message.content.clone(),
            timestamp: entry.at.to_rfc3339(),
        };
        let msg = serde_json::to_string(&WsResponse::ConversationEntry(item)).unwrap();
        if socket.send(WsMessage::Text(msg.into())).await.is_err() {
            state.connections.fetch_sub(1, Ordering::SeqCst);
            info!("websocket disconnected early");
            return;
        }
    }
    loop {
        tokio::select! {
            evt = events.recv() => {
                match evt {
                    Ok(Event::Speech { text, audio }) => {
                        let payload = serde_json::to_string(&WsResponse::Say { words: text.clone(), audio }).unwrap();
                        if socket.send(WsMessage::Text(payload.into())).await.is_err() {
                            error!("failed sending speech");
                            break;
                        }
                        let entry = ConvEntry {
                            role: "assistant".into(),
                            content: text,
                            timestamp: Utc::now().to_rfc3339(),
                        };
                        let msg = serde_json::to_string(&WsResponse::ConversationEntry(entry)).unwrap();
                        if socket.send(WsMessage::Text(msg.into())).await.is_err() {
                            break;
                        }
                    }
                    Ok(Event::StreamChunk(chunk)) => {
                        let msg = serde_json::to_string(&WsResponse::Chunk(chunk)).unwrap();
                        if socket.send(WsMessage::Text(msg.into())).await.is_err() {
                            break;
                        }
                    }
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
            , transcript = asr_text_rx.recv(), if asr_open => {
                match transcript {
                    Some((text, occurred_at)) => {
                        info!(%text, "asr finalized transcript");
                        state.ear.hear_user_say_at(&text, occurred_at).await;
                        let entry = ConvEntry {
                            role: "user".into(),
                            content: text,
                            timestamp: occurred_at.to_rfc3339(),
                        };
                        let msg = serde_json::to_string(&WsResponse::ConversationEntry(entry)).unwrap();
                        if socket.send(WsMessage::Text(msg.into())).await.is_err() {
                            break;
                        }
                    }
                    None => {
                        asr_open = false;
                    }
                }
            }
            , wit = wits.recv() => {
                if let Ok(report) = wit {
                    let msg = serde_json::to_string(&WsResponse::Think(report)).unwrap();
                    if socket.send(WsMessage::Text(msg.into())).await.is_err() {
                        break;
                    }
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(WsMessage::Text(text))) => {
                        if let Some(req) = parse_ws_request(&text) {
                            match req {
                                WsRequest::Text { text: message, at } => {
                                    let occurred_at = parse_ws_at(at.as_deref());
                                    debug!(text_len = message.len(), "user message received");
                                    state
                                        .ear
                                        .hear_web_interface_type_at(&message, occurred_at)
                                        .await;
                                    let entry = ConvEntry {
                                        role: "user".into(),
                                        content: message,
                                        timestamp: occurred_at.to_rfc3339(),
                                    };
                                    let msg = serde_json::to_string(&WsResponse::ConversationEntry(entry)).unwrap();
                                    let _ = socket.send(WsMessage::Text(msg.into())).await;
                                }
                                WsRequest::Echo { text, at } => {
                                    let occurred_at = parse_ws_at(at.as_deref());
                                    trace!(text_len = text.len(), "played ack received");
                                    state.ear.hear_self_say_at(&text, occurred_at).await;
                                }
                                WsRequest::See { data, at } => {
                                    if let Some((mime, base64)) = parse_data_url(&data) {
                                        if base64.trim().is_empty() {
                                            trace!("blank image ignored");
                                            state.eye.sense(ImageData { mime, base64: String::new(), captured_at: at }).await;
                                        } else {
                                            trace!("image received");
                                            state.eye.sense(ImageData { mime, base64, captured_at: at }).await;
                                        }
                                    }
                                }
                                WsRequest::Hear { data, at } => {
                                    #[cfg(feature = "asr")]
                                    handle_hear_frame(&data, at.as_deref(), &asr_pcm_tx).await;
                                    #[cfg(not(feature = "asr"))]
                                    {
                                        let _ = at;
                                        trace!(
                                            mime = %data.mime,
                                            bytes = data.base64.len(),
                                            "audio fragment received; server-side ASR disabled"
                                        );
                                    }
                                }
                                WsRequest::Geolocate { mut data, at } => {
                                    trace!("geolocation received");
                                    data.observed_at = at;
                                    state.geo.sense(data).await;
                                }
                                WsRequest::Motion { mut data, at } => {
                                    trace!("browser motion received");
                                    data.observed_at = at;
                                    state.motion.sense(data).await;
                                }
                                WsRequest::SpeechPlayback { text, status, at } => {
                                    let occurred_at = parse_ws_at(at.as_deref());
                                    match status {
                                        shared::SpeechPlaybackStatus::Started => {
                                            state.ear.started_speaking(&text, occurred_at).await;
                                        }
                                        shared::SpeechPlaybackStatus::Finished
                                        | shared::SpeechPlaybackStatus::Interrupted => {
                                            state.ear.finished_speaking(&text, occurred_at).await;
                                        }
                                    }
                                }
                                WsRequest::Sense { .. } => {
                                    trace!("sense event received");
                                }
                                _ => {}
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

fn parse_ws_request(text: &str) -> Option<WsRequest> {
    serde_json::from_str::<WsRequest>(text)
        .ok()
        .or_else(|| parse_flat_ws_request(text))
}

fn parse_ws_at(at: Option<&str>) -> DateTime<Utc> {
    at.and_then(psyche::parse_observed_at)
        .unwrap_or_else(Utc::now)
}

fn parse_flat_ws_request(text: &str) -> Option<WsRequest> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    match value.get("type")?.as_str()? {
        "Text" => {
            let text = value.get("text")?.as_str()?.to_string();
            let at = value
                .get("at")
                .and_then(|at| at.as_str())
                .map(ToString::to_string);
            Some(WsRequest::Text { text, at })
        }
        "Echo" => {
            let text = value.get("text")?.as_str()?.to_string();
            let at = value
                .get("at")
                .and_then(|at| at.as_str())
                .map(ToString::to_string);
            Some(WsRequest::Echo { text, at })
        }
        "See" => {
            let data = value.get("data")?.as_str()?.to_string();
            let at = value
                .get("at")
                .and_then(|at| at.as_str())
                .map(ToString::to_string);
            Some(WsRequest::See { data, at })
        }
        "Hear" => {
            let data = serde_json::from_value(value.get("data")?.clone()).ok()?;
            let at = value
                .get("at")
                .and_then(|at| at.as_str())
                .map(ToString::to_string);
            Some(WsRequest::Hear { data, at })
        }
        "Geolocate" => {
            let data = serde_json::from_value(value.get("data")?.clone()).ok()?;
            let at = value
                .get("at")
                .and_then(|at| at.as_str())
                .map(ToString::to_string);
            Some(WsRequest::Geolocate { data, at })
        }
        "Motion" => {
            let data = serde_json::from_value(value.get("data")?.clone()).ok()?;
            let at = value
                .get("at")
                .and_then(|at| at.as_str())
                .map(ToString::to_string);
            Some(WsRequest::Motion { data, at })
        }
        "Sense" => Some(WsRequest::Sense {
            data: value.get("data")?.clone(),
        }),
        "SpeechPlayback" => {
            let text = value.get("text")?.as_str()?.to_string();
            let status = serde_json::from_value(value.get("status")?.clone()).ok()?;
            let at = value
                .get("at")
                .and_then(|at| at.as_str())
                .map(ToString::to_string);
            Some(WsRequest::SpeechPlayback { text, status, at })
        }
        _ => None,
    }
}

#[cfg(feature = "asr")]
async fn handle_hear_frame(
    data: &shared::AudioData,
    at: Option<&str>,
    asr_pcm_tx: &Option<mpsc::Sender<crate::asr::AudioChunk>>,
) {
    let Some(tx) = asr_pcm_tx else {
        trace!(
            mime = %data.mime,
            bytes = data.base64.len(),
            "audio fragment received; Whisper model not configured"
        );
        return;
    };
    if !is_pcm_mime(&data.mime) {
        debug!(
            mime = %data.mime,
            bytes = data.base64.len(),
            "audio fragment ignored; expected 16-bit mono PCM"
        );
        return;
    }
    if let Some(channels) = data.channels {
        if channels != 1 {
            warn!(channels, "ASR expects mono PCM; forwarding chunk anyway");
        }
    }
    let bytes = match BASE64_STANDARD.decode(data.base64.trim().as_bytes()) {
        Ok(bytes) => bytes,
        Err(error) => {
            warn!(%error, "failed to decode ASR pcm payload");
            return;
        }
    };
    if bytes.is_empty() {
        return;
    }
    let chunk = crate::asr::AudioChunk {
        bytes,
        captured_at: parse_ws_at(at),
    };
    match tx.try_send(chunk) {
        Ok(()) => {}
        Err(mpsc::error::TrySendError::Full(_)) => {
            warn!("dropping ASR pcm chunk because processor queue is full");
        }
        Err(mpsc::error::TrySendError::Closed(_)) => {
            warn!("ASR processor is closed");
        }
    }
}

#[cfg(feature = "asr")]
fn is_pcm_mime(mime: &str) -> bool {
    let lower = mime.to_ascii_lowercase();
    lower.starts_with("audio/pcm") || lower.starts_with("audio/l16") || lower.contains("format=s16")
}

async fn handle_log_socket(mut socket: WebSocket, state: Body) {
    debug!("log websocket connected");
    let mut logs = state.bus.subscribe_logs();
    while let Ok(line) = logs.recv().await {
        if socket.send(WsMessage::Text(line.into())).await.is_err() {
            break;
        }
    }
    debug!("log websocket disconnected");
}

async fn handle_wit_socket(mut socket: WebSocket, state: Body) {
    debug!("wit websocket connected");
    for last in state.bus.latest_wits() {
        let msg = serde_json::to_string(&WsResponse::Think(last)).unwrap();
        if socket.send(WsMessage::Text(msg.into())).await.is_err() {
            return;
        }
    }
    let mut rx = state.bus.subscribe_wits();
    while let Ok(report) = rx.recv().await {
        let msg = serde_json::to_string(&WsResponse::Think(report.clone())).unwrap();
        if socket.send(WsMessage::Text(msg.into())).await.is_err() {
            break;
        }
    }
    debug!("wit websocket disconnected");
}

pub async fn conversation_log(State(state): State<Body>) -> impl IntoResponse {
    let conv = state.conversation.lock().await;
    let prompt = state.system_prompt.lock().await.clone();
    #[derive(Serialize)]
    struct Entry {
        role: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        timestamp: Option<DateTime<Utc>>,
    }
    let mut entries = vec![Entry {
        role: "system".to_string(),
        content: prompt,
        timestamp: None,
    }];
    entries.extend(conv.all_with_timestamps().iter().map(|m| Entry {
        role: match m.message.role {
            Role::User => "user".to_string(),
            Role::Assistant => "assistant".to_string(),
        },
        content: m.message.content.clone(),
        timestamp: Some(m.at),
    }));
    axum::Json(entries)
}

pub async fn psyche_debug(State(state): State<Body>) -> impl IntoResponse {
    let info = state.psyche_debug.snapshot().await;
    axum::Json(info)
}

#[derive(Deserialize)]
pub struct ToggleDebug {
    enable: bool,
}

pub async fn toggle_wit_debug(
    Path(label): Path<String>,
    Json(ToggleDebug { enable }): Json<ToggleDebug>,
) -> impl IntoResponse {
    if enable {
        psyche::enable_debug(&label).await;
    } else {
        psyche::disable_debug(&label).await;
    }
    StatusCode::OK
}

pub async fn wit_debug_page(Path(_label): Path<String>) -> Html<&'static str> {
    Html(include_str!("../../frontend/dist/wit_debug.html"))
}

/// Split a `data:` URL into its MIME type and base64 payload.
///
/// Returns `None` when the input is not a valid `data:` URL.
///
/// # Examples
/// ```
/// use pete::parse_data_url;
/// let url = "data:image/png;base64,Zm9v";
/// let (mime, data) = parse_data_url(url).unwrap();
/// assert_eq!(mime, "image/png");
/// assert_eq!(data, "Zm9v");
/// ```
pub fn parse_data_url(url: &str) -> Option<(String, String)> {
    let (prefix, data) = url.split_once(',')?;
    let mime = prefix
        .trim_start_matches("data:")
        .trim_end_matches(";base64");
    Some((mime.to_string(), data.to_string()))
}

/// Forward user text messages to the [`Ear`] and wake the [`Voice`].
///
/// Consumes messages from an [`mpsc::UnboundedReceiver`] and notifies the
/// ear of each line. After forwarding input the voice is permitted to speak.
///
/// ```
/// use pete::{listen_user_input, ChannelEar, dummy_psyche};
/// use std::sync::atomic::AtomicBool;
/// use tokio::sync::mpsc;
///
/// #[tokio::main]
/// async fn main() {
///     let mut psyche = dummy_psyche();
///     let ear = std::sync::Arc::new(ChannelEar::new(
///         psyche.input_sender(),
///         std::sync::Arc::new(AtomicBool::new(false)),
///         psyche.voice(),
///     ));
///     let (tx, rx) = mpsc::unbounded_channel();
///     tokio::spawn(listen_user_input(rx, ear, psyche.voice()));
///     tx.send("hello".into()).unwrap();
/// }
/// ```
pub async fn listen_user_input(
    mut rx: mpsc::UnboundedReceiver<String>,
    ear: Arc<dyn Ear>,
    voice: Arc<psyche::Voice>,
) {
    while let Some(msg) = rx.recv().await {
        debug!(text_len = msg.len(), "forwarding user input");
        ear.hear_user_say(&msg).await;
        voice.permit(None);
    }
}

pub fn app(state: Body) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/ws", get(ws_handler))
        .route("/log", get(log_ws_handler))
        .route("/debug", get(wit_ws_handler))
        .route(
            "/debug/wit/{label}",
            get(wit_debug_page).post(toggle_wit_debug),
        )
        .route("/debug/psyche", get(psyche_debug))
        .route("/conversation", get(conversation_log))
        .fallback_service(
            get_service(ServeDir::new("frontend/dist"))
                .handle_error(|_| async { StatusCode::INTERNAL_SERVER_ERROR }),
        )
        .with_state(state)
}

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
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, error, info};

use psyche::{Ear, Event};

#[derive(Clone)]
pub struct AppState {
    pub user_input: mpsc::UnboundedSender<String>,
    pub events: Arc<broadcast::Receiver<Event>>,
    pub ear: Arc<dyn Ear>,
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
}

#[derive(Serialize)]
struct WsResponse<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    text: String,
}

/// Serve the chat page rendered with Dioxus.
pub async fn index() -> Html<String> {
    use dioxus::prelude::*;
    use dioxus_ssr::render_lazy;

    info!("serving dioxus page");
    let raw = render_lazy(rsx! { div { dangerous_inner_html: include_str!("../page.html") } });
    let page = raw
        .trim_start_matches("<div>")
        .trim_end_matches("</div>")
        .to_string();
    Html(page)
}

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    info!("websocket upgrade initiated");
    ws.on_upgrade(move |socket| async move { handle_socket(socket, state).await })
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    info!("websocket connected");
    let mut events = state.events.resubscribe();
    loop {
        tokio::select! {
            evt = events.recv() => {
                match evt {
                    Ok(Event::StreamChunk(chunk)) => {
                        let payload = serde_json::to_string(&WsResponse { kind: "pete-says", text: chunk }).unwrap();
                        if socket.send(WsMessage::Text(payload.into())).await.is_err() {
                            error!("failed sending chunk");
                            break;
                        }
                    }
                    Ok(Event::IntentionToSay(text)) => {
                        let payload = serde_json::to_string(&WsResponse { kind: "pete-says", text: text.clone() }).unwrap();
                        if socket.send(WsMessage::Text(payload.into())).await.is_err() {
                            error!("failed sending intention");
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
                            }
                        }
                    }
                    Some(Ok(WsMessage::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
    info!("websocket disconnected");
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
        .with_state(state)
}

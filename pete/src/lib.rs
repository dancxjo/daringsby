use async_trait::async_trait;
use axum::{
    Router,
    extract::{
        State,
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
    },
    response::{Html, IntoResponse},
    routing::get,
};
use psyche::ling::{Chatter, Doer, Message, Vectorizer};
use psyche::{Event, Psyche, Sensation};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

#[derive(Clone)]
pub struct AppState {
    pub input: mpsc::UnboundedSender<Sensation>,
    pub events: Arc<broadcast::Receiver<Event>>,
}

#[derive(serde::Deserialize)]
struct WsRequest {
    message: String,
    #[allow(dead_code)]
    name: Option<String>,
}

#[derive(serde::Serialize)]
struct WsResponse<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    text: String,
}

/// Serve the embedded `index.html`.
pub async fn index() -> Html<&'static str> {
    static INDEX: &str = include_str!("../../index.html");
    Html(INDEX)
}

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move { handle_socket(socket, state).await })
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let mut events = state.events.resubscribe();
    loop {
        tokio::select! {
            evt = events.recv() => {
                match evt {
                    Ok(Event::StreamChunk(chunk)) => {
                        let payload = serde_json::to_string(&WsResponse { kind: "pete-says", text: chunk }).unwrap();
                        if socket.send(WsMessage::Text(payload.into())).await.is_err() { break; }
                    }
                    Ok(Event::IntentionToSay(text)) => {
                        let _ = state.input.send(Sensation::HeardOwnVoice(text.clone()));
                        let payload = serde_json::to_string(&WsResponse { kind: "pete-says", text }).unwrap();
                        if socket.send(WsMessage::Text(payload.into())).await.is_err() { break; }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(WsMessage::Text(text))) => {
                        if let Ok(req) = serde_json::from_str::<WsRequest>(&text) {
                            let _ = state.input.send(Sensation::HeardUserVoice(req.message));
                        }
                    }
                    Some(Ok(WsMessage::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}

/// Build the application router with the provided state.
pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/ws", get(ws_handler))
        .with_state(state)
}

/// Create a psyche with dummy providers for demos/tests.
pub fn dummy_psyche() -> Psyche {
    #[derive(Clone)]
    struct Dummy;

    #[async_trait]
    impl Doer for Dummy {
        async fn follow(&self, _: &str) -> anyhow::Result<String> {
            Ok("ok".into())
        }
    }

    #[async_trait]
    impl Chatter for Dummy {
        async fn chat(&self, _: &str, _: &[Message]) -> anyhow::Result<String> {
            Ok("hi".into())
        }
    }

    #[async_trait]
    impl Vectorizer for Dummy {
        async fn vectorize(&self, _: &str) -> anyhow::Result<Vec<f32>> {
            Ok(vec![0.0])
        }
    }

    let mut psyche = Psyche::new(Box::new(Dummy), Box::new(Dummy), Box::new(Dummy));
    psyche.set_turn_limit(10);
    psyche
}

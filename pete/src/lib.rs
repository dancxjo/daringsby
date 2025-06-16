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
use psyche::{Ear, Event, Mouth, Psyche, Sensation};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::{Mutex, broadcast, mpsc};

#[derive(Clone)]
pub struct AppState {
    pub user_input: mpsc::UnboundedSender<String>,
    pub events: Arc<broadcast::Receiver<Event>>,
    pub ear: Arc<dyn Ear>,
}

#[derive(serde::Deserialize)]
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

#[derive(serde::Serialize)]
struct WsResponse<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    text: String,
}

#[derive(Clone)]
pub struct ChannelEar {
    forward: mpsc::UnboundedSender<Sensation>,
    conversation: Arc<Mutex<psyche::Conversation>>, // share log from psyche
    speaking: Arc<AtomicBool>,
}

impl ChannelEar {
    pub fn new(
        forward: mpsc::UnboundedSender<Sensation>,
        conversation: Arc<Mutex<psyche::Conversation>>,
        speaking: Arc<AtomicBool>,
    ) -> Self {
        Self {
            forward,
            conversation,
            speaking,
        }
    }
}

#[async_trait]
impl Ear for ChannelEar {
    async fn hear_self_say(&self, text: &str) {
        self.speaking.store(false, Ordering::SeqCst);
        let _ = self
            .forward
            .send(Sensation::HeardOwnVoice(text.to_string()));
    }

    async fn hear_user_say(&self, text: &str) {
        let _ = self
            .forward
            .send(Sensation::HeardUserVoice(text.to_string()));
        let mut conv = self.conversation.lock().await;
        conv.add_user(text.to_string());
    }
}

#[derive(Clone)]
pub struct ChannelMouth {
    events: broadcast::Sender<Event>,
    speaking: Arc<AtomicBool>,
}

#[async_trait]
impl Mouth for ChannelMouth {
    async fn speak(&self, text: &str) {
        self.speaking.store(true, Ordering::SeqCst);
        let _ = self.events.send(Event::IntentionToSay(text.to_string()));
    }
    async fn interrupt(&self) {
        self.speaking.store(false, Ordering::SeqCst);
        let _ = self.events.send(Event::IntentionToSay(String::new()));
    }
    fn speaking(&self) -> bool {
        self.speaking.load(Ordering::SeqCst)
    }
}

#[derive(Clone)]
pub struct NoopMouth {
    speaking: Arc<AtomicBool>,
}

impl Default for NoopMouth {
    fn default() -> Self {
        Self {
            speaking: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[async_trait]
impl Mouth for NoopMouth {
    async fn speak(&self, _text: &str) {
        self.speaking.store(true, Ordering::SeqCst);
    }
    async fn interrupt(&self) {
        self.speaking.store(false, Ordering::SeqCst);
    }
    fn speaking(&self) -> bool {
        self.speaking.load(Ordering::SeqCst)
    }
}

#[derive(Clone)]
pub struct NoopEar;

#[async_trait]
impl Ear for NoopEar {
    async fn hear_self_say(&self, _text: &str) {}
    async fn hear_user_say(&self, _text: &str) {}
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
                        let payload = serde_json::to_string(&WsResponse { kind: "pete-says", text: text.clone() }).unwrap();
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
                            match req {
                                WsRequest::User { message, .. } => {
                                    let _ = state.user_input.send(message);
                                }
                                WsRequest::Displayed { text } => {
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

    let mouth = Arc::new(NoopMouth::default());
    let ear = Arc::new(NoopEar);
    let mut psyche = Psyche::new(
        Box::new(Dummy),
        Box::new(Dummy),
        Box::new(Dummy),
        mouth,
        ear,
    );
    psyche.set_turn_limit(10);
    psyche
}

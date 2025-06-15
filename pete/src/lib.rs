use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::State,
    response::{
        Html,
        sse::{Event as SseEvent, Sse},
    },
    routing::{get, post},
};
use psyche::ling::{Chatter, Doer, Message, Vectorizer};
use psyche::{Event, Psyche, Sensation};
use std::{convert::Infallible, sync::Arc};
use tokio::sync::{broadcast, mpsc};
use tokio_stream::{Stream, StreamExt, wrappers::BroadcastStream};

#[derive(Clone)]
pub struct AppState {
    pub input: mpsc::UnboundedSender<Sensation>,
    pub events: Arc<broadcast::Receiver<Event>>,
}

#[derive(serde::Deserialize)]
struct ChatRequest {
    message: String,
}

/// Serve the embedded `index.html`.
pub async fn index() -> Html<&'static str> {
    static INDEX: &str = include_str!("../../index.html");
    Html(INDEX)
}

pub async fn chat(
    State(state): State<AppState>,
    Json(payload): Json<ChatRequest>,
) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let _ = state.input.send(Sensation::HeardUserVoice(payload.message));

    let rx = state.events.resubscribe();
    let stream = BroadcastStream::new(rx).filter_map(|res| match res {
        Ok(Event::StreamChunk(chunk)) => Some(Ok(SseEvent::default().data(chunk))),
        _ => None,
    });

    Sse::new(stream)
}

/// Build the application router with the provided state.
pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/chat", post(chat))
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

    Psyche::new(Box::new(Dummy), Box::new(Dummy), Box::new(Dummy))
}

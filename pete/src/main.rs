use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::State,
    response::sse::{Event as SseEvent, Sse},
    routing::post,
};
use clap::Parser;
use psyche::ling::{Chatter, Doer, Message, Vectorizer};
use psyche::{Event, Psyche, Sensation};
use serde::Deserialize;
use std::{convert::Infallible, net::SocketAddr, sync::Arc};
use tokio::sync::{broadcast, mpsc};
use tokio_stream::{Stream, StreamExt, wrappers::BroadcastStream};

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    /// Address to bind the HTTP server
    #[arg(long, default_value = "127.0.0.1:3000")]
    addr: String,
}

#[derive(Clone)]
struct AppState {
    input: mpsc::UnboundedSender<Sensation>,
    events: Arc<broadcast::Receiver<Event>>,
}

#[derive(Deserialize)]
struct ChatRequest {
    message: String,
}

async fn chat(
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

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

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

    let narrator = Dummy;
    let voice = Dummy;
    let vectorizer = Dummy;

    let mut psyche = Psyche::new(Box::new(narrator), Box::new(voice), Box::new(vectorizer));
    let input = psyche.input_sender();
    let events = Arc::new(psyche.subscribe());

    tokio::spawn(async move {
        psyche.run().await;
    });

    let state = AppState {
        input,
        events: events.clone(),
    };
    let app = Router::new().route("/chat", post(chat)).with_state(state);

    let addr: SocketAddr = cli.addr.parse()?;
    println!("Listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}

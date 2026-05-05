use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::{
    Json, Router,
    extract::{
        State,
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, get_service},
};
use clap::Parser;
use dotenvy::dotenv;
use pete::{EventBus, init_logging};
use psyche::{GraphSnapshot, Neo4jClient};
use serde::Serialize;
use tokio::time::interval;
use tower_http::services::ServeDir;
use tracing::{error, info, warn};

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Serve Psychic, a real-time browser for Pete's graph"
)]
struct Cli {
    /// Address to bind the HTTP server.
    #[arg(long, default_value = "127.0.0.1:3001")]
    addr: String,
    /// Neo4j bolt or HTTP URI.
    #[arg(long, env = "NEO4J_URI", default_value = "bolt://localhost:7687")]
    neo4j_uri: String,
    /// Neo4j username.
    #[arg(long, env = "NEO4J_USER", default_value = "neo4j")]
    neo4j_user: String,
    /// Neo4j password.
    #[arg(long, env = "NEO4J_PASS", default_value = "password")]
    neo4j_pass: String,
    /// Maximum graph nodes to include in each snapshot.
    #[arg(long, env = "PSYCHIC_GRAPH_LIMIT", default_value_t = 160)]
    graph_limit: usize,
    /// Snapshot refresh interval for WebSocket clients.
    #[arg(long, env = "PSYCHIC_REFRESH_MS", default_value_t = 1000)]
    refresh_ms: u64,
}

#[derive(Clone)]
struct PsychicState {
    graph: Arc<Neo4jClient>,
    graph_limit: usize,
    refresh: Duration,
}

#[derive(Serialize)]
#[serde(tag = "type", content = "data")]
enum PsychicMessage<'a> {
    GraphSnapshot(&'a GraphSnapshot),
    Error { message: String },
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let (bus, _user_rx) = EventBus::new();
    init_logging(bus.log_sender());
    dotenv().ok();

    let cli = Cli::parse();
    let state = PsychicState {
        graph: Arc::new(Neo4jClient::new(
            cli.neo4j_uri,
            cli.neo4j_user,
            cli.neo4j_pass,
        )),
        graph_limit: cli.graph_limit,
        refresh: Duration::from_millis(cli.refresh_ms.max(250)),
    };
    let addr: SocketAddr = cli.addr.parse()?;
    let app = app(state);
    info!(%addr, "psychic graph server listening");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}

fn app(state: PsychicState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/graph", get(graph_snapshot))
        .route("/ws", get(ws_handler))
        .fallback_service(
            get_service(ServeDir::new("frontend/psychic"))
                .handle_error(|_| async { StatusCode::INTERNAL_SERVER_ERROR }),
        )
        .with_state(state)
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../../../frontend/psychic/index.html"))
}

async fn graph_snapshot(State(state): State<PsychicState>) -> impl IntoResponse {
    match state.graph.graph_snapshot(state.graph_limit).await {
        Ok(snapshot) => Json(snapshot).into_response(),
        Err(err) => {
            error!(%err, "failed to load graph snapshot");
            (
                StatusCode::BAD_GATEWAY,
                Json(PsychicMessage::Error {
                    message: err.to_string(),
                }),
            )
                .into_response()
        }
    }
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<PsychicState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move { handle_socket(socket, state).await })
}

async fn handle_socket(mut socket: WebSocket, state: PsychicState) {
    info!("psychic websocket connected");
    let mut ticks = interval(state.refresh);
    loop {
        ticks.tick().await;
        match state.graph.graph_snapshot(state.graph_limit).await {
            Ok(snapshot) => {
                let Ok(text) = serde_json::to_string(&PsychicMessage::GraphSnapshot(&snapshot))
                else {
                    continue;
                };
                if socket.send(WsMessage::Text(text.into())).await.is_err() {
                    break;
                }
            }
            Err(err) => {
                warn!(%err, "failed to stream graph snapshot");
                let Ok(text) = serde_json::to_string(&PsychicMessage::Error {
                    message: err.to_string(),
                }) else {
                    continue;
                };
                if socket.send(WsMessage::Text(text.into())).await.is_err() {
                    break;
                }
            }
        }
    }
    info!("psychic websocket disconnected");
}

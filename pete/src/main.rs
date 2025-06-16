use clap::Parser;
use pete::{AppState, ChannelEar, app, listen_user_input, ollama_psyche};
use std::{
    net::SocketAddr,
    sync::{Arc, atomic::AtomicBool},
};
use tokio::sync::mpsc;
use tracing::info;
use tracing_subscriber::fmt::init as log_init;

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    /// Address to bind the HTTP server
    #[arg(long, default_value = "127.0.0.1:3000")]
    addr: String,
    /// URL of the Ollama server
    #[arg(long, default_value = "http://localhost:11434")]
    ollama_url: String,
    /// Model name to use with Ollama
    #[arg(long, default_value = "mistral")]
    model: String,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    log_init();
    let cli = Cli::parse();

    info!(%cli.addr, "starting server");

    let mut psyche = ollama_psyche(&cli.ollama_url, &cli.model)?;
    let events = Arc::new(psyche.subscribe());
    let speaking = Arc::new(AtomicBool::new(false));
    let ear = Arc::new(ChannelEar::new(
        psyche.input_sender(),
        psyche.conversation(),
        speaking.clone(),
    ));
    let (user_tx, user_rx) = mpsc::unbounded_channel();

    tokio::spawn(listen_user_input(user_rx, ear.clone()));

    tokio::spawn(async move {
        psyche.run().await;
    });

    let state = AppState {
        user_input: user_tx,
        events: events.clone(),
        ear: ear.clone(),
    };
    let app = app(state);

    let addr: SocketAddr = cli.addr.parse()?;
    info!(%addr, "listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}

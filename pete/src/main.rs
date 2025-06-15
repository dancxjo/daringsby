use clap::Parser;
use pete::{AppState, app, dummy_psyche};
use std::{net::SocketAddr, sync::Arc};

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    /// Address to bind the HTTP server
    #[arg(long, default_value = "127.0.0.1:3000")]
    addr: String,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let mut psyche = dummy_psyche();
    let input = psyche.input_sender();
    let events = Arc::new(psyche.subscribe());

    tokio::spawn(async move {
        psyche.run().await;
    });

    let state = AppState {
        input,
        events: events.clone(),
    };
    let app = app(state);

    let addr: SocketAddr = cli.addr.parse()?;
    println!("Listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}

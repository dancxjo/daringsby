use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use clap::{Parser, Subcommand};
use futures::SinkExt;
use mime_guess::MimeGuess;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::info;

#[derive(Parser)]
struct Cli {
    /// WebSocket endpoint
    #[arg(long, default_value = "ws://127.0.0.1:3000/ws")]
    ws: String,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Send a text message
    Text { msg: String },
    /// Send an image file
    Image { path: String },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();
    let (mut ws, _) = connect_async(&cli.ws).await?;
    info!("connected to {ws}", ws = cli.ws);
    let payload = match cli.cmd {
        Cmd::Text { msg } => serde_json::json!({"type":"text","data":msg}),
        Cmd::Image { path } => {
            let bytes = tokio::fs::read(&path).await?;
            let mime = MimeGuess::from_path(&path).first_or_octet_stream();
            let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
            let data = format!("data:{};base64,{}", mime.essence_str(), b64);
            serde_json::json!({"type":"see","data":data})
        }
    };
    ws.send(Message::Text(payload.to_string().into())).await?;
    Ok(())
}

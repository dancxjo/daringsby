use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use pete::EyeSensor;
use pete::{
    AppState, ChannelEar, ChannelMouth, app, init_logging, listen_user_input, ollama_psyche,
};
#[cfg(feature = "tts")]
use pete::{CoquiTts, TtsMouth};
#[cfg(feature = "tts")]
use psyche::PlainMouth;
use psyche::{Mouth, Sensor, TrimMouth};
use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize},
    },
};
use tokio::sync::mpsc;
use tracing::info;

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
    /// URL of the Coqui TTS server
    #[arg(long, default_value = "http://localhost:5002/api/tts")]
    tts_url: String,
    /// Optional speaker ID for the TTS voice
    #[arg(long)]
    tts_speaker_id: Option<String>,
    /// Optional language ID for the TTS voice
    #[arg(long)]
    tts_language_id: Option<String>,
    /// Path to TLS certificate in PEM format
    #[arg(long)]
    tls_cert: Option<String>,
    /// Path to TLS private key in PEM format
    #[arg(long)]
    tls_key: Option<String>,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let (log_tx, _log_rx) = tokio::sync::broadcast::channel(100);
    init_logging(log_tx.clone());
    let cli = Cli::parse();

    info!(%cli.addr, "starting server");

    let mut psyche = ollama_psyche(&cli.ollama_url, &cli.model)?;
    let speaking = Arc::new(AtomicBool::new(false));
    let connections = Arc::new(AtomicUsize::new(0));
    #[cfg(feature = "tts")]
    let base_mouth: Arc<dyn Mouth> = {
        let tts = Arc::new(TtsMouth::new(
            psyche.event_sender(),
            speaking.clone(),
            Arc::new(CoquiTts::new(
                cli.tts_url,
                cli.tts_speaker_id,
                cli.tts_language_id,
            )),
        )) as Arc<dyn Mouth>;
        Arc::new(PlainMouth::new(tts)) as Arc<dyn Mouth>
    };
    #[cfg(not(feature = "tts"))]
    let base_mouth: Arc<dyn Mouth> =
        Arc::new(ChannelMouth::new(psyche.event_sender(), speaking.clone())) as Arc<dyn Mouth>;
    let mouth = Arc::new(TrimMouth::new(base_mouth)) as Arc<dyn Mouth>;
    psyche.set_mouth(mouth.clone());
    psyche.set_emotion("😐");
    psyche.set_connection_counter(connections.clone());
    let events = Arc::new(psyche.subscribe());
    let conversation = psyche.conversation();
    let ear = Arc::new(ChannelEar::new(
        psyche.input_sender(),
        conversation.clone(),
        speaking.clone(),
    ));
    let eye = Arc::new(EyeSensor::new(psyche.input_sender()));
    psyche.add_sense(eye.description());
    let (user_tx, user_rx) = mpsc::unbounded_channel();

    let voice = psyche.voice();
    tokio::spawn(listen_user_input(user_rx, ear.clone(), voice.clone()));

    let wit_rx = psyche.wit_reports();
    tokio::spawn(async move {
        psyche.run().await;
    });

    let state = AppState {
        user_input: user_tx,
        events: events.clone(),
        logs: Arc::new(log_tx.subscribe()),
        wits: Arc::new(wit_rx),
        ear: ear.clone(),
        eye: eye.clone(),
        conversation,
        connections,
    };
    let app = app(state);

    let addr: SocketAddr = cli.addr.parse()?;
    info!(%addr, "listening");
    if let (Some(cert), Some(key)) = (cli.tls_cert.as_deref(), cli.tls_key.as_deref()) {
        let config = RustlsConfig::from_pem_file(cert, key).await?;
        axum_server::bind_rustls(addr, config)
            .serve(app.into_make_service())
            .await?;
    } else {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app.into_make_service()).await?;
    }
    Ok(())
}

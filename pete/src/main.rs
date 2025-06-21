use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use pete::{
    AppState, ChannelEar, ChannelMouth, app, init_logging, listen_user_input, ollama_psyche,
};
#[cfg(feature = "tts")]
use pete::{CoquiTts, TtsMouth};
use pete::{EyeSensor, GeoSensor};
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
    /// URL of the chatter Ollama server
    #[arg(long, env = "CHATTER_HOST", default_value = "http://localhost:11434")]
    chatter_host: String,
    /// Model name to use for chatter
    #[arg(long, env = "CHATTER_MODEL", default_value = "mistral")]
    chatter_model: String,
    /// URL of the wits Ollama server
    #[arg(long, env = "WITS_HOST", default_value = "http://localhost:11434")]
    wits_host: String,
    /// Model name to use for wits
    #[arg(long, env = "WITS_MODEL", default_value = "mistral")]
    wits_model: String,
    /// URL of the embeddings Ollama server
    #[arg(
        long,
        env = "EMBEDDINGS_HOST",
        default_value = "http://localhost:11434"
    )]
    embeddings_host: String,
    /// Model name to use for embeddings
    #[arg(long, env = "EMBEDDINGS_MODEL", default_value = "mistral")]
    embeddings_model: String,
    /// URL of the Coqui TTS server
    #[arg(
        long,
        env = "COQUI_URL",
        default_value = "http://localhost:5002/api/tts"
    )]
    tts_url: String,
    /// Optional speaker ID for the TTS voice
    #[arg(long, env = "SPEAKER")]
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
    /// Allow the voice to speak every N seconds automatically
    #[arg(long)]
    auto_voice: Option<u64>,
    /// URL of the Qdrant service
    #[arg(long, env = "QDRANT_URL", default_value = "http://localhost:6333")]
    qdrant_url: String,
    /// Neo4j bolt URI
    #[arg(long, env = "NEO4J_URI", default_value = "bolt://localhost:7687")]
    neo4j_uri: String,
    /// Neo4j username
    #[arg(long, env = "NEO4J_USER", default_value = "neo4j")]
    neo4j_user: String,
    /// Neo4j password
    #[arg(long, env = "NEO4J_PASS", default_value = "password")]
    neo4j_pass: String,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let (bus, user_rx) = pete::EventBus::new();
    let bus = Arc::new(bus);
    init_logging(bus.log_sender());
    let cli = Cli::parse();

    info!(%cli.addr, "starting server");

    let mut psyche = ollama_psyche(
        &cli.chatter_host,
        &cli.chatter_model,
        &cli.wits_host,
        &cli.wits_model,
        &cli.embeddings_host,
        &cli.embeddings_model,
        &cli.qdrant_url,
        &cli.neo4j_uri,
        &cli.neo4j_user,
        &cli.neo4j_pass,
    )?;
    psyche.enable_all_debug().await;
    let speaking = Arc::new(AtomicBool::new(false));
    let connections = Arc::new(AtomicUsize::new(0));
    #[cfg(feature = "tts")]
    let base_mouth: Arc<dyn Mouth> = {
        let tts = Arc::new(TtsMouth::new(
            bus.event_sender(),
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
        Arc::new(ChannelMouth::new(bus.clone(), speaking.clone())) as Arc<dyn Mouth>;
    let mouth = Arc::new(TrimMouth::new(base_mouth)) as Arc<dyn Mouth>;
    psyche.set_mouth(mouth.clone());
    psyche.set_emotion("😐");
    psyche.set_connection_counter(connections.clone());
    let conversation = psyche.conversation();
    let voice = psyche.voice();
    let ear = Arc::new(ChannelEar::new(
        psyche.input_sender(),
        speaking.clone(),
        voice.clone(),
    ));
    let eye = Arc::new(EyeSensor::new(psyche.input_sender()));
    psyche.add_sense(eye.description());
    let geo = Arc::new(GeoSensor::new(psyche.input_sender()));
    psyche.add_sense(geo.description());
    tokio::spawn(listen_user_input(user_rx, ear.clone(), voice.clone()));

    if let Some(secs) = cli.auto_voice {
        let v = voice.clone();
        tokio::spawn(async move {
            let dur = std::time::Duration::from_secs(secs);
            loop {
                tokio::time::sleep(dur).await;
                v.permit(None);
            }
        });
    }

    let mut wit_rx = psyche.wit_reports();
    let debug_handle = psyche.debug_handle();
    let bus_clone = bus.clone();
    tokio::spawn(async move {
        while let Ok(r) = wit_rx.recv().await {
            bus_clone.publish_wit(r);
        }
    });
    let mut event_rx = psyche.subscribe();
    let bus_events = bus.clone();
    tokio::spawn(async move {
        while let Ok(evt) = event_rx.recv().await {
            bus_events.publish_event(evt);
        }
    });
    let system_prompt = psyche.system_prompt();
    tokio::spawn(async move {
        psyche.run().await;
    });

    let state = AppState {
        bus: bus.clone(),
        ear: ear.clone(),
        eye: eye.clone(),
        geo: geo.clone(),
        conversation,
        connections,
        system_prompt: Arc::new(tokio::sync::Mutex::new(system_prompt)),
        psyche_debug: debug_handle,
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

use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use dotenvy::dotenv;
#[cfg(feature = "ear")]
use pete::ChannelEar;
#[cfg(feature = "eye")]
use pete::EyeSensor;
#[cfg(feature = "face")]
use pete::FaceSensor;
#[cfg(feature = "geo")]
use pete::GeoSensor;
use pete::HeartbeatSensor;
use pete::{Body, LoggingMotor, NoopEar, NoopMouth, app, init_logging, listen_user_input};
// helper for building Ollama providers
use pete::default_mouth;
use pete::ollama_provider_from_args;
use psyche::{Ear, GeoLoc, ImageData, Mouth, Sensation, Sensor, TrimMouth};
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
    /// Speaker ID for the TTS voice
    #[arg(long, env = "SPEAKER", default_value = "p123")]
    tts_speaker_id: String,
    /// Language ID for the TTS voice
    #[arg(long, default_value = "en")]
    tts_language_id: String,
    /// Path to TLS certificate in PEM format
    #[arg(long)]
    tls_cert: Option<String>,
    /// Path to TLS private key in PEM format
    #[arg(long)]
    tls_key: Option<String>,
    /// Allow the voice to speak every N seconds automatically
    #[arg(long)]
    auto_voice: Option<u64>,
    /// Disable the fallback <take_turn> when no Wit suggests one
    #[arg(long)]
    no_fallback_turn: bool,
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
    dotenv().ok();
    let _ = dbg!(std::env::var("CHATTER_MODEL"));
    let cli = Cli::parse();

    info!(%cli.addr, "starting server");

    use psyche::wits::{
        BasicMemory, Combobulator, FaceMemoryWit, FondDuCoeur, HeartWit, IdentityWit, MemoryWit,
        Neo4jClient, QdrantClient, Quick, VisionWit, Will,
    };

    let narrator = ollama_provider_from_args(&cli.chatter_host, &cli.chatter_model)?;
    let voice_provider = ollama_provider_from_args(&cli.chatter_host, &cli.chatter_model)?;
    let vectorizer = ollama_provider_from_args(&cli.embeddings_host, &cli.embeddings_model)?;

    let memory = Arc::new(BasicMemory {
        vectorizer: Arc::new(ollama_provider_from_args(
            &cli.embeddings_host,
            &cli.embeddings_model,
        )?),
        qdrant: QdrantClient::new(cli.qdrant_url.clone()),
        neo4j: Arc::new(Neo4jClient::new(
            cli.neo4j_uri.clone(),
            cli.neo4j_user.clone(),
            cli.neo4j_pass.clone(),
        )),
    });

    let mouth_placeholder = Arc::new(NoopMouth::default());
    let ear_placeholder = Arc::new(NoopEar);
    let mut psyche = psyche::Psyche::new(
        Box::new(narrator),
        Box::new(voice_provider.clone()),
        Box::new(vectorizer),
        memory.clone(),
        mouth_placeholder,
        ear_placeholder,
    );
    psyche.set_turn_limit(usize::MAX);
    psyche
        .voice()
        .set_prompt(psyche::ContextualPrompt::new(psyche.topic_bus()));

    let wit_tx = psyche.wit_sender();
    psyche.register_observing_wit(Arc::new(VisionWit::with_debug(
        Arc::new(ollama_provider_from_args(&cli.wits_host, &cli.wits_model)?),
        wit_tx.clone(),
    )));
    psyche.register_observing_wit(Arc::new(FaceMemoryWit::with_debug(wit_tx.clone())));
    psyche.register_observing_wit(Arc::new(Quick::with_debug(
        psyche.topic_bus(),
        Arc::new(ollama_provider_from_args(&cli.wits_host, &cli.wits_model)?),
        Some(wit_tx.clone()),
    )));
    psyche.register_typed_wit(Arc::new(Combobulator::with_debug(
        Arc::new(ollama_provider_from_args(&cli.wits_host, &cli.wits_model)?),
        Some(wit_tx.clone()),
    )));
    psyche.register_typed_wit(Arc::new(Will::with_debug(
        psyche.topic_bus(),
        Arc::new(ollama_provider_from_args(&cli.wits_host, &cli.wits_model)?),
        Some(wit_tx.clone()),
    )));
    psyche.register_typed_wit(Arc::new(MemoryWit::with_debug(
        memory.clone(),
        wit_tx.clone(),
    )));
    psyche.register_typed_wit(Arc::new(HeartWit::with_debug(
        Box::new(ollama_provider_from_args(&cli.wits_host, &cli.wits_model)?),
        Arc::new(LoggingMotor),
        wit_tx.clone(),
    )));
    psyche.register_typed_wit(Arc::new(IdentityWit::new(FondDuCoeur::with_debug(
        Box::new(ollama_provider_from_args(&cli.wits_host, &cli.wits_model)?),
        wit_tx.clone(),
    ))));
    for w in psyche.debug_handle().snapshot().await.active_wits {
        tracing::debug!(%w, "registered wit");
    }
    psyche.enable_all_debug().await;
    psyche.set_fallback_turn_enabled(!cli.no_fallback_turn);
    let speaking = Arc::new(AtomicBool::new(false));
    let connections = Arc::new(AtomicUsize::new(0));
    let base_mouth: Arc<dyn Mouth> = default_mouth(
        bus.clone(),
        speaking.clone(),
        cli.tts_url,
        Some(cli.tts_speaker_id.clone()),
        Some(cli.tts_language_id.clone()),
    );
    let mouth = Arc::new(TrimMouth::new(base_mouth)) as Arc<dyn Mouth>;
    psyche.set_mouth(mouth.clone());
    psyche.set_emotion("😐");
    psyche.set_connection_counter(connections.clone());
    let conversation = psyche.conversation();
    let voice = psyche.voice();
    #[cfg(feature = "ear")]
    let ear: Arc<dyn Ear> = {
        psyche.add_sense(ChannelEar::DESCRIPTION.into());
        Arc::new(ChannelEar::new(
            psyche.input_sender(),
            speaking.clone(),
            voice.clone(),
        )) as Arc<dyn Ear>
    };
    #[cfg(not(feature = "ear"))]
    let ear: Arc<dyn Ear> = Arc::new(NoopEar) as Arc<dyn Ear>;
    #[cfg(feature = "eye")]
    let (eye_tx, mut eye_rx) = mpsc::channel(16);
    #[cfg(feature = "eye")]
    let eye: Arc<dyn Sensor<ImageData>> = {
        let sensor = Arc::new(EyeSensor::new(eye_tx)) as Arc<dyn Sensor<ImageData>>;
        psyche.add_sense(sensor.describe().into());
        sensor
    };
    #[cfg(not(feature = "eye"))]
    let eye: Arc<dyn Sensor<ImageData>> = Arc::new(NoopSensor) as Arc<dyn Sensor<ImageData>>;

    #[cfg(feature = "face")]
    let face_sensor = Arc::new(FaceSensor::new(
        Arc::new(psyche::DummyDetector::default()),
        psyche::QdrantClient::default(),
        psyche.topic_bus(),
    ));
    #[cfg(feature = "face")]
    {
        psyche.add_sense(face_sensor.describe().into());
    }
    #[cfg(all(feature = "eye", feature = "face"))]
    {
        let forward = psyche.input_sender();
        let face_clone = face_sensor.clone();
        tokio::spawn(async move {
            while let Some(s) = eye_rx.recv().await {
                if let Sensation::Of(any) = &s {
                    if let Some(img) = any.downcast_ref::<ImageData>() {
                        face_clone.sense(img.clone()).await;
                    }
                }
                let _ = forward.send(s).await;
            }
        });
    }

    #[cfg(feature = "geo")]
    let geo: Arc<dyn Sensor<GeoLoc>> = {
        let g = Arc::new(GeoSensor::new(psyche.input_sender())) as Arc<dyn Sensor<GeoLoc>>;
        psyche.add_sense(g.describe().into());
        g
    };
    #[cfg(not(feature = "geo"))]
    let geo: Arc<dyn Sensor<GeoLoc>> = Arc::new(NoopSensor) as Arc<dyn Sensor<GeoLoc>>;

    let _heartbeat = HeartbeatSensor::new(psyche.input_sender());
    psyche.add_sense(
        "Heartbeat. This triggers a pulse every minute, like a ticking internal clock.".into(),
    );
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
    let system_prompt = psyche.described_system_prompt();
    psyche.set_system_prompt(system_prompt.clone());
    tokio::spawn(async move {
        psyche.run().await;
    });

    let state = Body {
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

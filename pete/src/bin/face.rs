use std::{
    collections::VecDeque,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};

use axum::{
    Router,
    extract::{
        State,
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, get_service},
};
use axum_server::tls_rustls::RustlsConfig;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use chrono::{DateTime, Utc};
use clap::Parser;
use dotenvy::dotenv;
#[cfg(feature = "tts")]
use pete::{CoquiTts, synthesize_speech_audio};
use pete::{EventBus, MediaEvent, init_logging, parse_data_url};
use psyche::{
    AudioClip, ImageData, Impression, Neo4jClient, Sensation, SensationGraphObserver,
    SensationObserver, Stimulus, image_content_id,
};
use shared::{SpeechPlaybackStatus, WsPayload};
use tokio::{io::AsyncWriteExt, net::UnixListener, sync::broadcast};
use tower_http::services::ServeDir;
use tracing::{error, info, trace, warn};

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Serve the face client and expose raw media over IPC"
)]
struct Cli {
    /// Address to bind the HTTP server.
    #[arg(long, default_value = "127.0.0.1:3000")]
    addr: String,
    /// Unix socket path used for newline-delimited JSON media events.
    #[arg(long, env = "FACE_IPC", default_value = "/tmp/daringsby-face.sock")]
    ipc: PathBuf,
    /// Neo4j bolt or HTTP URI.
    #[arg(long, env = "NEO4J_URI", default_value = "bolt://localhost:7687")]
    neo4j_uri: String,
    /// Neo4j username.
    #[arg(long, env = "NEO4J_USER", default_value = "neo4j")]
    neo4j_user: String,
    /// Neo4j password.
    #[arg(long, env = "NEO4J_PASS", default_value = "password")]
    neo4j_pass: String,
    /// Path to TLS certificate in PEM format.
    #[arg(long)]
    tls_cert: Option<String>,
    /// Path to TLS private key in PEM format.
    #[arg(long)]
    tls_key: Option<String>,
    /// RMS threshold below which an audio chunk is treated as silence.
    #[arg(long, env = "FACE_SILENCE_THRESHOLD", default_value_t = 0.015)]
    silence_threshold: f32,
    /// Required trailing silence before an audio line is flushed.
    #[arg(long, env = "FACE_SILENCE_MS", default_value_t = 1200)]
    silence_ms: u64,
    /// Minimum buffered audio before silence can flush a line.
    #[arg(long, env = "FACE_AUDIO_MIN_MS", default_value_t = 250)]
    audio_min_ms: u64,
    /// Maximum buffered audio before forced flush.
    #[arg(long, env = "FACE_AUDIO_MAX_MS", default_value_t = 8000)]
    audio_max_ms: u64,
    /// Poll interval for reflecting the latest combobulation emoji on the face.
    #[arg(
        long,
        env = "FACE_COMBOBULATION_EMOTION_POLL_MS",
        default_value_t = 1000
    )]
    combobulation_emotion_poll_ms: u64,
    /// Poll interval for speech intentions chosen by Will.
    #[arg(long, env = "FACE_SPEECH_POLL_MS", default_value_t = 1000)]
    speech_poll_ms: u64,
    /// URL of the Coqui TTS server used for Will speech.
    #[arg(
        long,
        env = "COQUI_URL",
        default_value = "http://localhost:5002/api/tts"
    )]
    tts_url: String,
    /// Speaker ID for the TTS voice.
    #[arg(long, env = "SPEAKER", default_value = "p228")]
    tts_speaker_id: String,
    /// Language ID for the TTS voice.
    #[arg(long, default_value = "en")]
    tts_language_id: String,
}

#[derive(Clone)]
struct FaceState {
    graph: Arc<SensationGraphObserver>,
    ipc: broadcast::Sender<MediaEvent>,
    emotes: broadcast::Sender<WsPayload>,
    latest_emote: Arc<Mutex<Option<String>>>,
    image_sequence: Arc<AtomicU64>,
    audio_line_sequence: Arc<AtomicU64>,
    audio_config: AudioLineConfig,
    connections: Arc<AtomicUsize>,
}

#[derive(Clone)]
struct AudioLineConfig {
    silence_threshold: f32,
    silence_duration: Duration,
    min_duration: Duration,
    max_duration: Duration,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let (bus, _user_rx) = EventBus::new();
    init_logging(bus.log_sender());
    dotenv().ok();

    let cli = Cli::parse();
    let graph_store = Arc::new(Neo4jClient::new(
        cli.neo4j_uri.clone(),
        cli.neo4j_user.clone(),
        cli.neo4j_pass.clone(),
    ));
    let emotes = broadcast::channel(64).0;
    let latest_emote = Arc::new(Mutex::new(None));
    let state = FaceState {
        graph: Arc::new(SensationGraphObserver::new(graph_store.clone())),
        ipc: broadcast::channel(1024).0,
        emotes,
        latest_emote,
        image_sequence: Arc::new(AtomicU64::new(0)),
        audio_line_sequence: Arc::new(AtomicU64::new(0)),
        audio_config: AudioLineConfig {
            silence_threshold: cli.silence_threshold,
            silence_duration: Duration::from_millis(cli.silence_ms),
            min_duration: Duration::from_millis(cli.audio_min_ms),
            max_duration: Duration::from_millis(cli.audio_max_ms),
        },
        connections: Arc::new(AtomicUsize::new(0)),
    };

    spawn_combobulation_emote_poller(
        graph_store.clone(),
        state.graph.clone(),
        state.emotes.clone(),
        state.latest_emote.clone(),
        Duration::from_millis(cli.combobulation_emotion_poll_ms.max(100)),
    );
    spawn_speech_intention_poller(
        graph_store,
        state.emotes.clone(),
        state.connections.clone(),
        Duration::from_millis(cli.speech_poll_ms.max(100)),
        cli.tts_url,
        Some(cli.tts_speaker_id),
        Some(cli.tts_language_id),
    );
    spawn_ipc_server(cli.ipc.clone(), state.ipc.clone()).await?;
    let app = app(state);
    let addr: SocketAddr = cli.addr.parse()?;
    info!(%addr, ipc = %cli.ipc.display(), "face capture server listening");

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

fn app(state: FaceState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/ws", get(ws_handler))
        .fallback_service(
            get_service(ServeDir::new("frontend/dist"))
                .handle_error(|_| async { StatusCode::INTERNAL_SERVER_ERROR }),
        )
        .with_state(state)
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../../../frontend/dist/index.html"))
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<FaceState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move { handle_socket(socket, state).await })
}

async fn handle_socket(mut socket: WebSocket, state: FaceState) {
    state.connections.fetch_add(1, Ordering::SeqCst);
    let mut audio_lines = AudioLineBuffer::new(state.audio_config.clone());
    let mut emotes = state.emotes.subscribe();
    info!(
        active = state.connections.load(Ordering::SeqCst),
        "face websocket connected"
    );

    let latest_emote = { state.latest_emote.lock().unwrap().clone() };
    if let Some(emoji) = latest_emote {
        let payload = serde_json::to_string(&WsPayload::Emote(emoji)).unwrap();
        if socket.send(WsMessage::Text(payload.into())).await.is_err() {
            state.connections.fetch_sub(1, Ordering::SeqCst);
            return;
        }
    }

    loop {
        tokio::select! {
            msg = socket.recv() => {
                match msg {
                    Some(Ok(WsMessage::Text(text))) => {
                        if let Some(request) = parse_ws_request(&text) {
                            handle_request(request, &state, &mut audio_lines).await;
                        }
                    }
                    Some(Ok(WsMessage::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(err)) => {
                        warn!(%err, "face websocket receive failed");
                        break;
                    }
                }
            }
            emote = emotes.recv() => {
                match emote {
                    Ok(payload) => {
                        let payload = serde_json::to_string(&payload).unwrap();
                        if socket.send(WsMessage::Text(payload.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    if let Some(line) = audio_lines.flush(Utc::now()) {
        store_audio_line(line, &state).await;
    }

    state.connections.fetch_sub(1, Ordering::SeqCst);
    info!(
        active = state.connections.load(Ordering::SeqCst),
        "face websocket disconnected"
    );
}

async fn handle_request(request: WsPayload, state: &FaceState, audio_lines: &mut AudioLineBuffer) {
    match request {
        WsPayload::See { data, at } => {
            let Some((mime, base64)) = parse_data_url(&data) else {
                warn!("invalid image data URL");
                return;
            };
            if base64.trim().is_empty() {
                trace!("blank image ignored");
                return;
            }

            let received_at = Utc::now();
            let occurred_at = parse_ws_at(at.as_deref()).unwrap_or(received_at);
            let image = ImageData {
                mime,
                base64,
                captured_at: Some(occurred_at.to_rfc3339()),
            };
            let content_id = image_content_id(&image);
            let sensation = Sensation::of_at(image.clone(), occurred_at);
            state.graph.observe_sensation(&sensation).await;

            let event = MediaEvent::Image {
                sequence: state.image_sequence.fetch_add(1, Ordering::SeqCst) + 1,
                mime: image.mime,
                base64: image.base64,
                content_id,
                captured_at: image.captured_at,
                received_at,
            };
            let _ = state.ipc.send(event);
        }
        WsPayload::Hear { data, at } => {
            let received_at = Utc::now();
            if !is_pcm_mime(&data.mime) {
                warn!(mime = %data.mime, "ignored audio frame; expected 16-bit PCM");
                return;
            }
            let captured_at = parse_ws_at(at.as_deref()).unwrap_or(received_at);
            let sample_rate = data.sample_rate.unwrap_or(16_000);
            let channels = data.channels.unwrap_or(1).clamp(1, u16::MAX as u32) as u16;
            match audio_lines.ingest_base64(
                data.base64.trim(),
                data.mime,
                sample_rate,
                channels,
                captured_at,
                received_at,
            ) {
                Ok(Some(line)) => store_audio_line(line, state).await,
                Ok(None) => {}
                Err(err) => warn!(%err, "failed to buffer audio frame"),
            }
        }
        WsPayload::Text { text, at } => {
            trace!(
                text_len = text.len(),
                "text received by face capture server"
            );
            let occurred_at = parse_ws_at(at.as_deref()).unwrap_or_else(Utc::now);
            let sensation = Sensation::web_interface_text_at(text, occurred_at);
            state.graph.observe_sensation(&sensation).await;
        }
        WsPayload::Echo { text, at } => {
            trace!(
                text_len = text.len(),
                "echo received by face capture server"
            );
            let occurred_at = parse_ws_at(at.as_deref()).unwrap_or_else(Utc::now);
            let sensation = Sensation::heard_own_voice_at(text, occurred_at);
            state.graph.observe_sensation(&sensation).await;
        }
        WsPayload::SpeechPlayback { text, status, at } => {
            trace!(
                text_len = text.len(),
                ?status,
                "speech playback event received by face capture server"
            );
            let occurred_at = parse_ws_at(at.as_deref()).unwrap_or_else(Utc::now);
            store_speech_playback_sensation(&state.graph, &text, status, occurred_at).await;
        }
        WsPayload::Geolocate { mut data, at } => {
            let received_at = Utc::now();
            let occurred_at = parse_ws_at(at.as_deref()).unwrap_or(received_at);
            if data.observed_at.is_none() {
                data.observed_at = Some(occurred_at.to_rfc3339());
            }
            let sensation = Sensation::of_at(data, occurred_at);
            state.graph.observe_sensation(&sensation).await;
        }
        WsPayload::Sense { data } => {
            let sensation = Sensation::of_at(data, Utc::now());
            state.graph.observe_sensation(&sensation).await;
        }
        _ => {}
    }
}

async fn store_audio_line(line: CompletedAudioLine, state: &FaceState) {
    let captured_at = line.started_at.to_rfc3339();
    let clip = AudioClip {
        mime: line.mime.clone(),
        base64: line.base64.clone(),
        sample_rate: line.sample_rate,
        channels: line.channels,
        transcript: None,
        captured_at: Some(captured_at),
    };
    let sensation = Sensation::of_at(clip, line.started_at);
    state.graph.observe_sensation(&sensation).await;

    let event = MediaEvent::AudioLine {
        sequence: state.audio_line_sequence.fetch_add(1, Ordering::SeqCst) + 1,
        mime: line.mime,
        base64: line.base64,
        sample_rate: line.sample_rate,
        channels: line.channels,
        started_at: line.started_at,
        ended_at: line.ended_at,
        duration_ms: line.duration_ms,
        received_at: line.received_at,
    };
    let _ = state.ipc.send(event);
}

struct AudioLineBuffer {
    config: AudioLineConfig,
    samples: VecDeque<i16>,
    mime: String,
    sample_rate: u32,
    channels: u16,
    started_at: Option<DateTime<Utc>>,
    last_received_at: Option<DateTime<Utc>>,
    tail_silence_samples: usize,
}

struct CompletedAudioLine {
    mime: String,
    base64: String,
    sample_rate: u32,
    channels: u16,
    started_at: DateTime<Utc>,
    ended_at: DateTime<Utc>,
    duration_ms: u64,
    received_at: DateTime<Utc>,
}

impl AudioLineBuffer {
    fn new(config: AudioLineConfig) -> Self {
        Self {
            config,
            samples: VecDeque::new(),
            mime: "audio/pcm;format=s16le;rate=16000".to_string(),
            sample_rate: 16_000,
            channels: 1,
            started_at: None,
            last_received_at: None,
            tail_silence_samples: 0,
        }
    }

    fn ingest_base64(
        &mut self,
        base64: &str,
        mime: String,
        sample_rate: u32,
        channels: u16,
        captured_at: DateTime<Utc>,
        received_at: DateTime<Utc>,
    ) -> anyhow::Result<Option<CompletedAudioLine>> {
        let bytes = BASE64_STANDARD.decode(base64.as_bytes())?;
        if bytes.is_empty() {
            return Ok(None);
        }
        if self.samples.is_empty() {
            self.mime = mime;
            self.sample_rate = sample_rate;
            self.channels = channels;
            self.started_at = Some(captured_at);
            self.tail_silence_samples = 0;
        }
        self.last_received_at = Some(received_at);

        let mut sum_sq = 0.0f64;
        let mut count = 0usize;
        for chunk in bytes.chunks_exact(2) {
            let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
            let normalized = sample as f32 / i16::MAX as f32;
            sum_sq += f64::from(normalized) * f64::from(normalized);
            self.samples.push_back(sample);
            count += 1;
        }
        if count == 0 {
            return Ok(None);
        }

        let rms = (sum_sq / count as f64).sqrt() as f32;
        if rms <= self.config.silence_threshold {
            self.tail_silence_samples = self.tail_silence_samples.saturating_add(count);
        } else {
            self.tail_silence_samples = 0;
        }

        if self.should_flush() {
            return Ok(self.flush(received_at));
        }
        Ok(None)
    }

    fn should_flush(&self) -> bool {
        let min_samples = duration_samples(
            self.config.min_duration,
            self.sample_rate,
            u32::from(self.channels),
        );
        let max_samples = duration_samples(
            self.config.max_duration,
            self.sample_rate,
            u32::from(self.channels),
        );
        let silence_samples = duration_samples(
            self.config.silence_duration,
            self.sample_rate,
            u32::from(self.channels),
        );
        self.samples.len() >= max_samples
            || (self.samples.len() >= min_samples && self.tail_silence_samples >= silence_samples)
    }

    fn flush(&mut self, received_at: DateTime<Utc>) -> Option<CompletedAudioLine> {
        if self.samples.is_empty() {
            return None;
        }

        trim_silence(
            &mut self.samples,
            self.sample_rate,
            u32::from(self.channels),
            self.config.silence_threshold,
        );
        if self.samples.is_empty() {
            self.reset();
            return None;
        }

        let started_at = self.started_at.unwrap_or_else(Utc::now);
        let duration_ms = (self.samples.len() as u64 * 1000)
            / u64::from(self.sample_rate * u32::from(self.channels));
        let ended_at = started_at + chrono::Duration::milliseconds(duration_ms as i64);
        let mut bytes = Vec::with_capacity(self.samples.len() * 2);
        for sample in self.samples.drain(..) {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }

        let line = CompletedAudioLine {
            mime: self.mime.clone(),
            base64: BASE64_STANDARD.encode(bytes),
            sample_rate: self.sample_rate,
            channels: self.channels,
            started_at,
            ended_at,
            duration_ms,
            received_at: self.last_received_at.unwrap_or(received_at),
        };
        self.reset();
        Some(line)
    }

    fn reset(&mut self) {
        self.started_at = None;
        self.last_received_at = None;
        self.tail_silence_samples = 0;
    }
}

fn duration_samples(duration: Duration, sample_rate: u32, channels: u32) -> usize {
    (duration.as_secs_f32() * sample_rate as f32 * channels as f32).round() as usize
}

fn trim_silence(samples: &mut VecDeque<i16>, sample_rate: u32, channels: u32, threshold: f32) {
    let window = ((sample_rate * channels) / 50).max(1) as usize;
    while samples.len() >= window && rms(samples.iter().take(window).copied()) <= threshold {
        for _ in 0..window {
            samples.pop_front();
        }
    }
    while samples.len() >= window && rms(samples.iter().rev().take(window).copied()) <= threshold {
        for _ in 0..window {
            samples.pop_back();
        }
    }
}

fn rms(samples: impl Iterator<Item = i16>) -> f32 {
    let mut sum_sq = 0.0f64;
    let mut count = 0usize;
    for sample in samples {
        let normalized = sample as f32 / i16::MAX as f32;
        sum_sq += f64::from(normalized) * f64::from(normalized);
        count += 1;
    }
    if count == 0 {
        0.0
    } else {
        (sum_sq / count as f64).sqrt() as f32
    }
}

fn is_pcm_mime(mime: &str) -> bool {
    let lower = mime.to_ascii_lowercase();
    lower.starts_with("audio/pcm") || lower.starts_with("audio/l16") || lower.contains("format=s16")
}

fn spawn_combobulation_emote_poller(
    graph: Arc<Neo4jClient>,
    observer: Arc<SensationGraphObserver>,
    tx: broadcast::Sender<WsPayload>,
    latest: Arc<Mutex<Option<String>>>,
    poll_interval: Duration,
) {
    tokio::spawn(async move {
        let mut last_id: Option<String> = None;
        loop {
            match graph.latest_presentable_face_emotion().await {
                Ok(Some(emotion)) if last_id.as_deref() != Some(emotion.id.as_str()) => {
                    last_id = Some(emotion.id);
                    *latest.lock().unwrap() = Some(emotion.emoji.clone());
                    store_face_report_sensation(&observer, &emotion.emoji).await;
                    let _ = tx.send(WsPayload::Emote(emotion.emoji));
                }
                Ok(_) => {}
                Err(err) => warn!(%err, "failed polling latest face emotion"),
            }
            tokio::time::sleep(poll_interval).await;
        }
    });
}

async fn store_face_report_sensation(observer: &SensationGraphObserver, emoji: &str) {
    let occurred_at = Utc::now();
    let summary = format!("I feel my face turn into a {emoji}.");
    let impression = Impression::new(
        vec![Stimulus::at(
            format!("face display changed to {emoji}"),
            occurred_at,
        )],
        summary,
        Some(emoji.to_string()),
    );
    let sensation = Sensation::of_at(impression, occurred_at);
    observer.observe_sensation(&sensation).await;
}

fn spawn_speech_intention_poller(
    graph: Arc<Neo4jClient>,
    tx: broadcast::Sender<WsPayload>,
    connections: Arc<AtomicUsize>,
    poll_interval: Duration,
    tts_url: String,
    speaker_id: Option<String>,
    language_id: Option<String>,
) {
    tokio::spawn(async move {
        let mut last_id: Option<String> = None;
        #[cfg(feature = "tts")]
        let tts = CoquiTts::new(tts_url, speaker_id, language_id);
        #[cfg(not(feature = "tts"))]
        let tts = {
            let _ = (tts_url, speaker_id, language_id);
            ()
        };
        loop {
            if connections.load(Ordering::SeqCst) == 0 {
                tokio::time::sleep(poll_interval).await;
                continue;
            }
            match graph.latest_pending_speech_intention().await {
                Ok(Some(intention)) if last_id.as_deref() != Some(intention.id.as_str()) => {
                    last_id = Some(intention.id);
                    let audio = speech_audio(&intention.text, &tts).await;
                    let _ = tx.send(WsPayload::Say {
                        words: intention.text,
                        audio,
                    });
                }
                Ok(_) => {}
                Err(err) => warn!(%err, "failed polling latest speech intention"),
            }
            tokio::time::sleep(poll_interval).await;
        }
    });
}

#[cfg(feature = "tts")]
async fn speech_audio(text: &str, tts: &CoquiTts) -> Option<String> {
    match synthesize_speech_audio(tts, text).await {
        Ok(audio) => audio,
        Err(err) => {
            warn!(%err, "tts request failed for will speech");
            None
        }
    }
}

#[cfg(not(feature = "tts"))]
async fn speech_audio(_text: &str, _tts: &()) -> Option<String> {
    None
}

async fn store_speech_playback_sensation(
    observer: &SensationGraphObserver,
    text: &str,
    status: SpeechPlaybackStatus,
    occurred_at: DateTime<Utc>,
) {
    let verb = match status {
        SpeechPlaybackStatus::Started => "start",
        SpeechPlaybackStatus::Finished => "finish",
        SpeechPlaybackStatus::Interrupted => "stop",
    };
    let summary = format!("I {verb} saying: {}", text.trim());
    let impression = Impression::new(
        vec![Stimulus::at(
            format!("speech playback {verb}: {}", text.trim()),
            occurred_at,
        )],
        summary,
        None::<String>,
    );
    let sensation = Sensation::of_at(impression, occurred_at);
    observer.observe_sensation(&sensation).await;
}

async fn spawn_ipc_server(path: PathBuf, tx: broadcast::Sender<MediaEvent>) -> anyhow::Result<()> {
    remove_stale_socket(&path)?;
    let listener = UnixListener::bind(&path)?;
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((mut stream, _addr)) => {
                    let mut rx = tx.subscribe();
                    tokio::spawn(async move {
                        while let Ok(event) = rx.recv().await {
                            let Ok(mut line) = serde_json::to_vec(&event) else {
                                continue;
                            };
                            line.push(b'\n');
                            if stream.write_all(&line).await.is_err() {
                                break;
                            }
                        }
                    });
                }
                Err(err) => {
                    error!(%err, "face IPC accept failed");
                    break;
                }
            }
        }
    });
    Ok(())
}

fn remove_stale_socket(path: &Path) -> anyhow::Result<()> {
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

fn parse_ws_request(text: &str) -> Option<WsPayload> {
    serde_json::from_str::<WsPayload>(text)
        .ok()
        .or_else(|| parse_flat_ws_request(text))
}

fn parse_flat_ws_request(text: &str) -> Option<WsPayload> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    match value.get("type")?.as_str()? {
        "Text" => {
            let data = value.get("data");
            let text = data
                .and_then(|data| data.get("text"))
                .or_else(|| value.get("text"))?
                .as_str()?
                .to_string();
            let at = data
                .and_then(|data| data.get("at"))
                .or_else(|| value.get("at"))
                .and_then(|at| at.as_str())
                .map(ToString::to_string);
            Some(WsPayload::Text { text, at })
        }
        "Echo" => {
            let text = value.get("text")?.as_str()?.to_string();
            let at = value
                .get("at")
                .and_then(|at| at.as_str())
                .map(ToString::to_string);
            Some(WsPayload::Echo { text, at })
        }
        "SpeechPlayback" => {
            let text = value.get("text")?.as_str()?.to_string();
            let status = serde_json::from_value(value.get("status")?.clone()).ok()?;
            let at = value
                .get("at")
                .and_then(|at| at.as_str())
                .map(ToString::to_string);
            Some(WsPayload::SpeechPlayback { text, status, at })
        }
        "See" => {
            let data = value.get("data")?.as_str()?.to_string();
            let at = value
                .get("at")
                .and_then(|at| at.as_str())
                .map(ToString::to_string);
            Some(WsPayload::See { data, at })
        }
        "Hear" => {
            let data = serde_json::from_value(value.get("data")?.clone()).ok()?;
            let at = value
                .get("at")
                .and_then(|at| at.as_str())
                .map(ToString::to_string);
            Some(WsPayload::Hear { data, at })
        }
        "Geolocate" => {
            let data = serde_json::from_value(value.get("data")?.clone()).ok()?;
            let at = value
                .get("at")
                .and_then(|at| at.as_str())
                .map(ToString::to_string);
            Some(WsPayload::Geolocate { data, at })
        }
        "Sense" => Some(WsPayload::Sense {
            data: value.get("data")?.clone(),
        }),
        _ => None,
    }
}

fn parse_ws_at(at: Option<&str>) -> Option<DateTime<Utc>> {
    at.and_then(psyche::parse_observed_at)
}

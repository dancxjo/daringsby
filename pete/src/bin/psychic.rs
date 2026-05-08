use std::{
    fs,
    net::SocketAddr,
    path::{Path as FsPath, PathBuf},
    sync::Arc,
    time::Duration,
};

use axum::{
    Json, Router,
    extract::{
        Path, State,
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
    },
    http::{StatusCode, header},
    response::{Html, IntoResponse},
    routing::{get, get_service, post},
};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64_STANDARD};
use chrono::{DateTime, NaiveDateTime, Utc};
use clap::Parser;
use dotenvy::dotenv;
use pete::{EventBus, init_logging, movie};
use psyche::{AudioClip, GraphNodeDetails, GraphSnapshot, GraphSpeechSegmentAudio, Neo4jClient};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::time::interval;
use tower_http::services::ServeDir;
use tracing::{error, info, warn};

const DEFAULT_MOVIE_DIR: &str = "movies";
const SPEECH_SEGMENT_PREROLL_MS: u32 = 40;
const SPEECH_SEGMENT_POSTROLL_MS: u32 = 90;
const SPEECH_SEGMENT_EDGE_SEARCH_MS: u32 = 12;
const SPEECH_SEGMENT_FADE_MS: u32 = 6;

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
    #[arg(long, env = "PSYCHIC_GRAPH_LIMIT", default_value_t = 5000)]
    graph_limit: usize,
    /// Snapshot refresh interval for WebSocket clients.
    #[arg(long, env = "PSYCHIC_REFRESH_MS", default_value_t = 1000)]
    refresh_ms: u64,
    /// Maximum duration the browser may request for generated movies.
    #[arg(long, env = "PSYCHIC_MOVIE_MAX_MS", default_value_t = 180_000)]
    movie_max_ms: i64,
}

#[derive(Clone)]
struct PsychicState {
    graph: Arc<Neo4jClient>,
    graph_limit: usize,
    refresh: Duration,
    movie_dir: PathBuf,
    movie_max_ms: i64,
    movie_render_lock: Arc<Mutex<()>>,
}

#[derive(Serialize)]
#[serde(tag = "type", content = "data")]
enum PsychicMessage<'a> {
    GraphSnapshot(&'a GraphSnapshot),
    Error { message: String },
}

#[derive(Serialize)]
struct MovieAsset {
    src: String,
    captions: Option<String>,
    from: String,
    to: String,
    duration_ms: i64,
}

#[derive(Deserialize)]
struct MovieRequest {
    from: String,
    to: String,
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
        movie_dir: PathBuf::from(DEFAULT_MOVIE_DIR),
        movie_max_ms: cli.movie_max_ms.max(1000),
        movie_render_lock: Arc::new(Mutex::new(())),
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
        .route("/movie-index", get(movie_index))
        .route("/movie", post(request_movie))
        .route("/graph/node/{id}", get(graph_node_details))
        .route("/graph/audio-clip/{id}/audio.wav", get(audio_clip_audio))
        .route(
            "/graph/speech-segment/{id}/audio.wav",
            get(speech_segment_audio),
        )
        .route("/ws", get(ws_handler))
        .nest_service(
            "/movies",
            get_service(ServeDir::new(DEFAULT_MOVIE_DIR))
                .handle_error(|_| async { StatusCode::INTERNAL_SERVER_ERROR }),
        )
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

async fn movie_index() -> impl IntoResponse {
    match movie_assets(FsPath::new(DEFAULT_MOVIE_DIR)) {
        Ok(assets) => Json(assets).into_response(),
        Err(err) => {
            error!(%err, "failed to index movies");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(PsychicMessage::Error {
                    message: err.to_string(),
                }),
            )
                .into_response()
        }
    }
}

async fn request_movie(
    State(state): State<PsychicState>,
    Json(request): Json<MovieRequest>,
) -> impl IntoResponse {
    match provide_movie(&state, request).await {
        Ok(asset) => Json(asset).into_response(),
        Err(err) => {
            warn!(%err, "failed to provide requested movie");
            (
                StatusCode::BAD_REQUEST,
                Json(PsychicMessage::Error {
                    message: err.to_string(),
                }),
            )
                .into_response()
        }
    }
}

async fn provide_movie(state: &PsychicState, request: MovieRequest) -> anyhow::Result<MovieAsset> {
    let from = movie::parse_time(&request.from)?;
    let to = movie::parse_time(&request.to)?;
    anyhow::ensure!(to > from, "movie request must have a positive duration");
    let duration_ms = (to - from).num_milliseconds();
    anyhow::ensure!(
        duration_ms <= state.movie_max_ms,
        "movie request duration {duration_ms}ms exceeds the {}ms limit",
        state.movie_max_ms
    );

    let out = state.movie_dir.join(format!(
        "pete-{}-{}.webm",
        movie_time_for_path(from),
        movie_time_for_path(to)
    ));
    let work_dir = movie::default_work_dir(&out);
    let _guard = state.movie_render_lock.lock().await;
    if !out.exists() {
        movie::render_graph_movie(&state.graph, out.clone(), work_dir, from, to).await?;
    }
    movie_asset_for_path(&state.movie_dir, &out)
}

fn movie_assets(root: &FsPath) -> anyhow::Result<Vec<MovieAsset>> {
    let mut assets = Vec::new();
    if !root.exists() {
        return Ok(assets);
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("webm") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let Some((from, to)) = movie_times_from_stem(stem) else {
            continue;
        };
        let captions_path = path.with_extension("vtt");
        let captions = captions_url(&captions_path);
        assets.push(MovieAsset {
            src: movie_url(&path),
            captions,
            from: from.to_rfc3339(),
            to: to.to_rfc3339(),
            duration_ms: (to - from).num_milliseconds(),
        });
    }
    assets.sort_by(|left, right| left.from.cmp(&right.from));
    Ok(assets)
}

fn movie_asset_for_path(root: &FsPath, path: &FsPath) -> anyhow::Result<MovieAsset> {
    anyhow::ensure!(
        path.starts_with(root),
        "movie path is outside movie directory"
    );
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| anyhow::anyhow!("movie path is missing a file stem"))?;
    let (from, to) = movie_times_from_stem(stem)
        .ok_or_else(|| anyhow::anyhow!("movie path has an invalid timestamp range"))?;
    Ok(MovieAsset {
        src: movie_url(path),
        captions: captions_url(&path.with_extension("vtt")),
        from: from.to_rfc3339(),
        to: to.to_rfc3339(),
        duration_ms: (to - from).num_milliseconds(),
    })
}

fn movie_url(path: &FsPath) -> String {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    format!("/movies/{file_name}")
}

fn captions_url(path: &FsPath) -> Option<String> {
    path.exists()
        .then(|| {
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string()
        })
        .filter(|name| !name.is_empty())
        .map(|name| format!("/movies/{name}"))
}

fn movie_times_from_stem(stem: &str) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let rest = stem.strip_prefix("pete-")?;
    let (from, to) = rest.split_once('-')?;
    Some((parse_movie_time(from)?, parse_movie_time(to)?))
}

fn movie_time_for_path(value: DateTime<Utc>) -> String {
    value.format("%Y%m%dT%H%M%SZ").to_string()
}

fn parse_movie_time(value: &str) -> Option<DateTime<Utc>> {
    let naive = NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%SZ").ok()?;
    Some(DateTime::from_naive_utc_and_offset(naive, Utc))
}

async fn graph_node_details(
    Path(id): Path<String>,
    State(state): State<PsychicState>,
) -> impl IntoResponse {
    match state.graph.graph_node_details(&id).await {
        Ok(Some(details)) => Json(details).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(PsychicMessage::Error {
                message: format!("graph node not found: {id}"),
            }),
        )
            .into_response(),
        Err(err) => {
            error!(%err, id = %id, "failed to load graph node details");
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

async fn audio_clip_audio(
    Path(id): Path<String>,
    State(state): State<PsychicState>,
) -> impl IntoResponse {
    let lookup_id = decoded_path_id(&id);
    match load_audio_clip(&state, &id, lookup_id.as_deref()).await {
        Ok(Some(clip)) => match audio_clip_wav(&clip) {
            Ok(wav) => ([(header::CONTENT_TYPE, "audio/wav")], wav).into_response(),
            Err(err) => {
                error!(%err, id = %id, "failed to create audio clip WAV");
                (
                    StatusCode::BAD_REQUEST,
                    Json(PsychicMessage::Error {
                        message: err.to_string(),
                    }),
                )
                    .into_response()
            }
        },
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(PsychicMessage::Error {
                message: format!("audio clip not found: {id}"),
            }),
        )
            .into_response(),
        Err(err) => {
            error!(%err, id = %id, "failed to load audio clip");
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

async fn speech_segment_audio(
    Path(id): Path<String>,
    State(state): State<PsychicState>,
) -> impl IntoResponse {
    let lookup_id = decoded_path_id(&id);
    match load_speech_segment_audio(&state, &id, lookup_id.as_deref()).await {
        Ok(Some(segment)) => match speech_segment_wav(&segment) {
            Ok(wav) => ([(header::CONTENT_TYPE, "audio/wav")], wav).into_response(),
            Err(err) => {
                error!(%err, id = %id, "failed to create speech segment audio");
                (
                    StatusCode::BAD_REQUEST,
                    Json(PsychicMessage::Error {
                        message: err.to_string(),
                    }),
                )
                    .into_response()
            }
        },
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(PsychicMessage::Error {
                message: format!("speech segment audio not found: {id}"),
            }),
        )
            .into_response(),
        Err(err) => {
            error!(%err, id = %id, "failed to load speech segment audio");
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

async fn load_audio_clip(
    state: &PsychicState,
    id: &str,
    decoded_id: Option<&str>,
) -> anyhow::Result<Option<AudioClip>> {
    if let Some(clip) = load_audio_clip_by_id(state, id).await? {
        return Ok(Some(clip));
    }
    let Some(decoded_id) = decoded_id.filter(|decoded_id| *decoded_id != id) else {
        return Ok(None);
    };
    load_audio_clip_by_id(state, decoded_id).await
}

async fn load_audio_clip_by_id(
    state: &PsychicState,
    id: &str,
) -> anyhow::Result<Option<AudioClip>> {
    let Some(details) = state.graph.graph_node_details(id).await? else {
        return Ok(None);
    };
    audio_clip_from_details(details)
}

fn audio_clip_from_details(details: GraphNodeDetails) -> anyhow::Result<Option<AudioClip>> {
    if !details.labels.iter().any(|label| label == "AudioClip") {
        return Ok(None);
    }
    serde_json::from_value(details.properties)
        .map(Some)
        .map_err(|err| anyhow::anyhow!("failed to decode audio clip properties: {err}"))
}

async fn load_speech_segment_audio(
    state: &PsychicState,
    id: &str,
    decoded_id: Option<&str>,
) -> anyhow::Result<Option<GraphSpeechSegmentAudio>> {
    let segment = state.graph.graph_speech_segment_audio(id).await?;
    if segment.is_some() {
        return Ok(segment);
    }
    let Some(decoded_id) = decoded_id.filter(|decoded_id| *decoded_id != id) else {
        return Ok(None);
    };
    state.graph.graph_speech_segment_audio(decoded_id).await
}

fn decoded_path_id(id: &str) -> Option<String> {
    urlencoding::decode(id)
        .ok()
        .map(|decoded| decoded.into_owned())
        .filter(|decoded| decoded != id)
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

struct PcmAudio {
    bytes: Vec<u8>,
    sample_rate: u32,
    channels: u16,
}

fn speech_segment_wav(segment: &GraphSpeechSegmentAudio) -> anyhow::Result<Vec<u8>> {
    anyhow::ensure!(
        segment.end_ms > segment.start_ms,
        "speech segment has invalid timing {}..{}ms",
        segment.start_ms,
        segment.end_ms
    );
    let source_bytes = BASE64_STANDARD
        .decode(segment.base64.trim().as_bytes())
        .map_err(|err| anyhow::anyhow!("failed to decode source audio base64: {err}"))?;
    let source = decode_source_audio(
        &segment.mime,
        &source_bytes,
        segment.sample_rate,
        segment.channels,
    )?;
    let frame_size = usize::from(source.channels).saturating_mul(2);
    anyhow::ensure!(frame_size > 0, "source audio has no channels");
    let frame_count = source.bytes.len() / frame_size;
    let requested_start = segment.start_ms.saturating_sub(SPEECH_SEGMENT_PREROLL_MS);
    let requested_end = segment.end_ms.saturating_add(SPEECH_SEGMENT_POSTROLL_MS);
    let start_frame = ms_to_frame(requested_start, source.sample_rate).min(frame_count);
    let end_frame = ms_to_frame(requested_end, source.sample_rate).min(frame_count);
    anyhow::ensure!(
        end_frame > start_frame,
        "speech segment falls outside source audio"
    );
    let search_frames = ms_to_frame(SPEECH_SEGMENT_EDGE_SEARCH_MS, source.sample_rate);
    let start_frame = quiet_frame_near(&source.bytes, source.channels, start_frame, search_frames);
    let end_frame = quiet_frame_near(&source.bytes, source.channels, end_frame, search_frames);
    let end_frame = end_frame.max(start_frame + 1).min(frame_count);
    let start_byte = start_frame * frame_size;
    let end_byte = end_frame * frame_size;
    let mut pcm = source.bytes[start_byte..end_byte].to_vec();
    let fade_frames = ms_to_frame(SPEECH_SEGMENT_FADE_MS, source.sample_rate);
    apply_edge_fades(&mut pcm, source.channels, fade_frames);
    Ok(encode_wav_pcm_s16le(
        &pcm,
        source.sample_rate,
        source.channels,
    ))
}

fn audio_clip_wav(clip: &AudioClip) -> anyhow::Result<Vec<u8>> {
    let source_bytes = BASE64_STANDARD
        .decode(clip.base64.trim().as_bytes())
        .map_err(|err| anyhow::anyhow!("failed to decode audio clip base64: {err}"))?;
    let source = decode_source_audio(&clip.mime, &source_bytes, clip.sample_rate, clip.channels)?;
    Ok(encode_wav_pcm_s16le(
        &source.bytes,
        source.sample_rate,
        source.channels,
    ))
}

fn decode_source_audio(
    mime: &str,
    bytes: &[u8],
    sample_rate: u32,
    channels: u16,
) -> anyhow::Result<PcmAudio> {
    let mime = mime.to_ascii_lowercase();
    if mime.starts_with("audio/wav")
        || mime.starts_with("audio/x-wav")
        || bytes.starts_with(b"RIFF")
    {
        return decode_wav_pcm_s16le(bytes);
    }
    if is_pcm_s16_mime(&mime) {
        return Ok(PcmAudio {
            bytes: bytes.to_vec(),
            sample_rate,
            channels,
        });
    }
    anyhow::bail!("unsupported source audio MIME type {mime}");
}

fn decode_wav_pcm_s16le(bytes: &[u8]) -> anyhow::Result<PcmAudio> {
    anyhow::ensure!(bytes.len() >= 12, "WAV data is too short");
    anyhow::ensure!(
        bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WAVE",
        "source audio is not a RIFF/WAVE file"
    );
    let mut offset = 12usize;
    let mut format = None;
    let mut data = None;
    while offset + 8 <= bytes.len() {
        let chunk_id = &bytes[offset..offset + 4];
        let chunk_size = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]) as usize;
        let chunk_start = offset + 8;
        let chunk_end = chunk_start.saturating_add(chunk_size);
        anyhow::ensure!(
            chunk_end <= bytes.len(),
            "WAV chunk extends past end of data"
        );
        match chunk_id {
            b"fmt " => {
                anyhow::ensure!(chunk_size >= 16, "WAV fmt chunk is too short");
                let audio_format = u16::from_le_bytes([bytes[chunk_start], bytes[chunk_start + 1]]);
                let channels = u16::from_le_bytes([bytes[chunk_start + 2], bytes[chunk_start + 3]]);
                let sample_rate = u32::from_le_bytes([
                    bytes[chunk_start + 4],
                    bytes[chunk_start + 5],
                    bytes[chunk_start + 6],
                    bytes[chunk_start + 7],
                ]);
                let bits_per_sample =
                    u16::from_le_bytes([bytes[chunk_start + 14], bytes[chunk_start + 15]]);
                format = Some((audio_format, channels, sample_rate, bits_per_sample));
            }
            b"data" => {
                data = Some(bytes[chunk_start..chunk_end].to_vec());
            }
            _ => {}
        }
        offset = chunk_end + (chunk_size % 2);
    }
    let (audio_format, channels, sample_rate, bits_per_sample) =
        format.ok_or_else(|| anyhow::anyhow!("WAV data is missing a fmt chunk"))?;
    anyhow::ensure!(
        audio_format == 1,
        "only PCM WAV speech segments are supported"
    );
    anyhow::ensure!(
        bits_per_sample == 16,
        "only 16-bit WAV speech segments are supported"
    );
    Ok(PcmAudio {
        bytes: data.ok_or_else(|| anyhow::anyhow!("WAV data is missing a data chunk"))?,
        sample_rate,
        channels,
    })
}

fn is_pcm_s16_mime(mime: &str) -> bool {
    mime.starts_with("audio/pcm") || mime.starts_with("audio/l16") || mime.contains("format=s16")
}

fn ms_to_frame(ms: u32, sample_rate: u32) -> usize {
    (u128::from(ms).saturating_mul(u128::from(sample_rate)) / 1000) as usize
}

fn quiet_frame_near(pcm: &[u8], channels: u16, target_frame: usize, radius_frames: usize) -> usize {
    let frame_size = usize::from(channels).saturating_mul(2);
    if frame_size == 0 {
        return target_frame;
    }
    let frame_count = pcm.len() / frame_size;
    if frame_count == 0 {
        return 0;
    }
    let target_frame = target_frame.min(frame_count);
    let start = target_frame.saturating_sub(radius_frames);
    let end = target_frame.saturating_add(radius_frames).min(frame_count);
    (start..=end)
        .filter(|frame| is_zero_crossing_boundary(pcm, channels, *frame))
        .min_by_key(|frame| frame.abs_diff(target_frame))
        .unwrap_or(target_frame)
}

fn is_zero_crossing_boundary(pcm: &[u8], channels: u16, frame: usize) -> bool {
    let frame_size = usize::from(channels).saturating_mul(2);
    let frame_count = pcm.len() / frame_size;
    if frame == 0 || frame >= frame_count {
        return true;
    }
    let previous = frame_signal(pcm, channels, frame - 1);
    let current = frame_signal(pcm, channels, frame);
    previous == 0 || current == 0 || previous.signum() != current.signum()
}

fn frame_signal(pcm: &[u8], channels: u16, frame: usize) -> i32 {
    let frame_size = usize::from(channels).saturating_mul(2);
    let start = frame.saturating_mul(frame_size);
    let end = start.saturating_add(frame_size).min(pcm.len());
    pcm[start..end]
        .chunks_exact(2)
        .map(|sample| i32::from(i16::from_le_bytes([sample[0], sample[1]])))
        .sum()
}

fn apply_edge_fades(pcm: &mut [u8], channels: u16, fade_frames: usize) {
    let frame_size = usize::from(channels).saturating_mul(2);
    if frame_size == 0 || fade_frames == 0 {
        return;
    }
    let frame_count = pcm.len() / frame_size;
    let fade_frames = fade_frames.min(frame_count / 2);
    if fade_frames == 0 {
        return;
    }
    for frame in 0..fade_frames {
        let fade_in = frame as f32 / fade_frames as f32;
        scale_frame(pcm, frame_size, frame, fade_in);

        let end_frame = frame_count - frame - 1;
        let fade_out = frame as f32 / fade_frames as f32;
        scale_frame(pcm, frame_size, end_frame, fade_out);
    }
}

fn scale_frame(pcm: &mut [u8], frame_size: usize, frame: usize, gain: f32) {
    let start = frame.saturating_mul(frame_size);
    let end = start.saturating_add(frame_size).min(pcm.len());
    for sample in pcm[start..end].chunks_exact_mut(2) {
        let value = i16::from_le_bytes([sample[0], sample[1]]) as f32;
        let scaled = (value * gain)
            .round()
            .clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        sample.copy_from_slice(&scaled.to_le_bytes());
    }
}

fn encode_wav_pcm_s16le(pcm: &[u8], sample_rate: u32, channels: u16) -> Vec<u8> {
    let mut wav = Vec::with_capacity(44 + pcm.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36u32.saturating_add(pcm.len() as u32)).to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(
        &(sample_rate
            .saturating_mul(u32::from(channels))
            .saturating_mul(2))
        .to_le_bytes(),
    );
    wav.extend_from_slice(&channels.saturating_mul(2).to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    wav.extend_from_slice(pcm);
    wav
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speech_segment_wav_keeps_context_around_raw_pcm_timing() {
        let pcm = [1i16, 2, 3, 4]
            .into_iter()
            .flat_map(i16::to_le_bytes)
            .collect::<Vec<_>>();
        let segment = GraphSpeechSegmentAudio {
            segment_id: "speech:1".into(),
            text: "two three".into(),
            audio_clip_id: "audio:1".into(),
            mime: "audio/pcm;format=s16le;rate=1000".into(),
            base64: BASE64_STANDARD.encode(&pcm),
            sample_rate: 1000,
            channels: 1,
            start_ms: 1,
            end_ms: 3,
        };

        let wav = speech_segment_wav(&segment).unwrap();

        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 8);
        assert_eq!(pcm_i16_samples(&wav[44..]), vec![0, 1, 2, 0]);
    }

    #[test]
    fn speech_segment_wav_keeps_context_around_wav_sources() {
        let pcm = [1i16, 2, 3, 4]
            .into_iter()
            .flat_map(i16::to_le_bytes)
            .collect::<Vec<_>>();
        let source_wav = encode_wav_pcm_s16le(&pcm, 1000, 1);
        let segment = GraphSpeechSegmentAudio {
            segment_id: "speech:1".into(),
            text: "three".into(),
            audio_clip_id: "audio:1".into(),
            mime: "audio/wav".into(),
            base64: BASE64_STANDARD.encode(&source_wav),
            sample_rate: 1000,
            channels: 1,
            start_ms: 2,
            end_ms: 3,
        };

        let wav = speech_segment_wav(&segment).unwrap();

        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 8);
        assert_eq!(pcm_i16_samples(&wav[44..]), vec![0, 1, 2, 0]);
    }

    #[test]
    fn audio_clip_wav_wraps_raw_pcm() {
        let pcm = [1i16, -2, 3, -4]
            .into_iter()
            .flat_map(i16::to_le_bytes)
            .collect::<Vec<_>>();
        let clip = AudioClip {
            mime: "audio/pcm;format=s16le;rate=1000".into(),
            base64: BASE64_STANDARD.encode(&pcm),
            sample_rate: 1000,
            channels: 1,
            transcript: None,
            captured_at: None,
        };

        let wav = audio_clip_wav(&clip).unwrap();

        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 8);
        assert_eq!(pcm_i16_samples(&wav[44..]), vec![1, -2, 3, -4]);
    }

    #[test]
    fn quiet_frame_near_prefers_zero_crossing_boundaries() {
        let pcm = [10i16, 10, 10, -10, -10]
            .into_iter()
            .flat_map(i16::to_le_bytes)
            .collect::<Vec<_>>();

        let frame = quiet_frame_near(&pcm, 1, 2, 2);

        assert_eq!(frame, 3);
    }

    #[test]
    fn decoded_path_id_accepts_encoded_segment_ids() {
        let id = "transcription%3Asha256%3Aabc%3Asegment%3A3";

        assert_eq!(
            decoded_path_id(id).as_deref(),
            Some("transcription:sha256:abc:segment:3")
        );
    }

    fn pcm_i16_samples(pcm: &[u8]) -> Vec<i16> {
        pcm.chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect()
    }
}

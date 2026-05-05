use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::{
    Json, Router,
    extract::{
        Path, State,
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
    },
    http::{StatusCode, header},
    response::{Html, IntoResponse},
    routing::{get, get_service},
};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64_STANDARD};
use clap::Parser;
use dotenvy::dotenv;
use pete::{EventBus, init_logging};
use psyche::{GraphSnapshot, GraphSpeechSegmentAudio, Neo4jClient};
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
        .route("/graph/node/{id}", get(graph_node_details))
        .route(
            "/graph/speech-segment/{id}/audio.wav",
            get(speech_segment_audio),
        )
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

async fn speech_segment_audio(
    Path(id): Path<String>,
    State(state): State<PsychicState>,
) -> impl IntoResponse {
    match state.graph.graph_speech_segment_audio(&id).await {
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
    let start_frame = ms_to_frame(segment.start_ms, source.sample_rate).min(frame_count);
    let end_frame = ms_to_frame(segment.end_ms, source.sample_rate).min(frame_count);
    anyhow::ensure!(
        end_frame > start_frame,
        "speech segment falls outside source audio"
    );
    let start_byte = start_frame * frame_size;
    let end_byte = end_frame * frame_size;
    Ok(encode_wav_pcm_s16le(
        &source.bytes[start_byte..end_byte],
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
    fn speech_segment_wav_slices_raw_pcm_by_timing() {
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
        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 4);
        assert_eq!(&wav[44..], &pcm[2..6]);
    }

    #[test]
    fn speech_segment_wav_slices_wav_sources() {
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

        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 2);
        assert_eq!(&wav[44..], &pcm[4..6]);
    }
}

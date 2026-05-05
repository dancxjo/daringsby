use std::time::Duration;

use anyhow::Context;
use chrono::{DateTime, Utc};
use clap::Parser;
use dotenvy::dotenv;
use pete::{AsrService, EventBus, SegmentMessage, init_logging};
use psyche::{GraphAudioClip, GraphSpeechSegment, Neo4jClient, parse_observed_at};
use tokio::time::{MissedTickBehavior, interval};
use tracing::{debug, error, info};

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Transcribe stored AudioClip graph nodes with Whisper"
)]
struct Cli {
    /// Neo4j bolt or HTTP URI.
    #[arg(long, env = "NEO4J_URI", default_value = "bolt://localhost:7687")]
    neo4j_uri: String,
    /// Neo4j username.
    #[arg(long, env = "NEO4J_USER", default_value = "neo4j")]
    neo4j_user: String,
    /// Neo4j password.
    #[arg(long, env = "NEO4J_PASS", default_value = "password")]
    neo4j_pass: String,
    /// Delay between graph polling attempts.
    #[arg(long, env = "TRANSCRIPTION_POLL_MS", default_value_t = 1000)]
    poll_ms: u64,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let (bus, _user_rx) = EventBus::new();
    init_logging(bus.log_sender());
    dotenv().ok();

    let cli = Cli::parse();
    let Some(asr) = AsrService::from_env()? else {
        anyhow::bail!("Whisper model not configured; set WHISPER_MODEL or run `just fetch`");
    };
    if !asr.has_whisper_model() {
        anyhow::bail!("Whisper model not configured; set WHISPER_MODEL or run `just fetch`");
    }
    let graph = Neo4jClient::new(cli.neo4j_uri, cli.neo4j_user, cli.neo4j_pass);
    let mut ticker = interval(Duration::from_millis(cli.poll_ms.max(100)));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    info!("transcription loop started");
    loop {
        ticker.tick().await;
        if let Err(err) = transcribe_next_clip(&graph, &asr).await {
            error!(error = %err, "transcription loop iteration failed");
        }
    }
}

async fn transcribe_next_clip(graph: &Neo4jClient, asr: &AsrService) -> anyhow::Result<()> {
    let Some(audio) = graph
        .latest_untranscribed_audio_clip()
        .await
        .context("failed to load latest untranscribed audio clip")?
    else {
        debug!("no untranscribed audio clips found");
        return Ok(());
    };

    info!(clip_id = %audio.id, "transcribing audio clip");
    let transcription = asr
        .transcribe_clip(&audio.clip)
        .await
        .with_context(|| format!("failed to transcribe audio clip {}", audio.id))?;
    let source_started_at = audio_timestamp(&audio);
    let source_captured_at = source_started_at.map(|at| at.to_rfc3339());
    let segments = graph_speech_segments(&transcription.segments, source_started_at);
    graph
        .attach_audio_transcription(
            &audio.id,
            &transcription.text,
            source_captured_at.as_deref(),
            &segments,
        )
        .await
        .with_context(|| format!("failed to attach transcription to audio clip {}", audio.id))?;
    info!(
        clip_id = %audio.id,
        transcript_len = transcription.text.len(),
        segment_count = transcription.segments.len(),
        "attached audio transcription"
    );
    Ok(())
}

fn audio_timestamp(audio: &GraphAudioClip) -> Option<DateTime<Utc>> {
    audio
        .clip
        .captured_at
        .as_deref()
        .and_then(parse_observed_at)
        .or_else(|| audio.occurred_at.as_deref().and_then(parse_observed_at))
}

fn graph_speech_segments(
    segments: &[SegmentMessage],
    source_started_at: Option<DateTime<Utc>>,
) -> Vec<GraphSpeechSegment> {
    segments
        .iter()
        .enumerate()
        .map(|(index, segment)| {
            let occurred_at = source_started_at
                .map(|at| at + chrono::Duration::milliseconds(i64::from(segment.start_ms)))
                .map(|at| at.to_rfc3339());
            let ended_at = source_started_at
                .map(|at| at + chrono::Duration::milliseconds(i64::from(segment.end_ms)))
                .map(|at| at.to_rfc3339());
            GraphSpeechSegment {
                index,
                text: segment.text.clone(),
                start_ms: segment.start_ms,
                end_ms: segment.end_ms,
                occurred_at,
                ended_at,
            }
        })
        .collect()
}

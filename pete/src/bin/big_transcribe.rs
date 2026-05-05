use std::time::Duration;

use anyhow::Context;
use chrono::{DateTime, Utc};
use clap::Parser;
use dotenvy::dotenv;
use pete::{AsrService, EventBus, SegmentMessage, SourceClipSpan, WordTiming, init_logging};
use psyche::{
    GraphAudioClip, GraphAudioClipWindow, GraphAudioSourceSpan, GraphSpeechSegment, Neo4jClient,
    parse_observed_at,
};
use tokio::time::{MissedTickBehavior, interval};
use tracing::{debug, error, info};

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Retranscribe recent AudioClip graph windows with Whisper"
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
    #[arg(long, env = "BIG_TRANSCRIPTION_POLL_MS", default_value_t = 1000)]
    poll_ms: u64,
    /// Number of recent audio clips to join into one transcription pass.
    #[arg(long, env = "BIG_TRANSCRIPTION_WINDOW_SIZE", default_value_t = 4)]
    window_size: usize,
    /// Minimum number of clips required before an aggregate transcription runs.
    #[arg(long, env = "BIG_TRANSCRIPTION_MIN_CLIPS", default_value_t = 2)]
    min_clips: usize,
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
    let window_size = cli.window_size.max(cli.min_clips).max(1);
    let min_clips = cli.min_clips.max(1);

    info!(window_size, min_clips, "big transcription loop started");
    loop {
        ticker.tick().await;
        if let Err(err) = transcribe_next_window(&graph, &asr, window_size, min_clips).await {
            error!(error = %err, "big transcription loop iteration failed");
        }
    }
}

async fn transcribe_next_window(
    graph: &Neo4jClient,
    asr: &AsrService,
    window_size: usize,
    min_clips: usize,
) -> anyhow::Result<()> {
    let Some(window) = graph
        .latest_audio_clip_window_for_big_transcription(window_size)
        .await
        .context("failed to load latest audio clip window")?
    else {
        debug!("no audio clip windows found for big transcription");
        return Ok(());
    };

    if window.clips.len() < min_clips {
        debug!(
            anchor_id = %window.anchor_id,
            clip_count = window.clips.len(),
            min_clips,
            "waiting for more audio clips before big transcription"
        );
        return Ok(());
    }

    let audio_clips = window
        .clips
        .iter()
        .map(|clip| clip.clip.clone())
        .collect::<Vec<_>>();
    info!(
        anchor_id = %window.anchor_id,
        clip_count = window.clips.len(),
        "big transcribing audio clip window"
    );
    let transcription = asr
        .transcribe_clips(&audio_clips)
        .await
        .with_context(|| format!("failed to big transcribe audio window {}", window.anchor_id))?;

    let source_started_at = window.clips.first().and_then(audio_timestamp);
    let source_ended_at = source_started_at.and_then(|started_at| {
        transcription
            .source_spans
            .last()
            .map(|span| started_at + chrono::Duration::milliseconds(i64::from(span.end_ms)))
    });
    let source_started_at_text = source_started_at.map(|at| at.to_rfc3339());
    let source_ended_at_text = source_ended_at.map(|at| at.to_rfc3339());
    let sources = graph_source_spans(&window, &transcription.source_spans);
    let segments = graph_speech_segments(&transcription.segments, source_started_at);

    graph
        .attach_big_audio_transcription(
            &sources,
            &transcription.text,
            source_started_at_text.as_deref(),
            source_ended_at_text.as_deref(),
            &segments,
        )
        .await
        .with_context(|| {
            format!(
                "failed to attach big transcription for audio window {}",
                window.anchor_id
            )
        })?;
    info!(
        anchor_id = %window.anchor_id,
        transcript_len = transcription.text.len(),
        segment_count = transcription.segments.len(),
        source_count = sources.len(),
        "attached big audio transcription"
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

fn graph_source_spans(
    window: &GraphAudioClipWindow,
    spans: &[SourceClipSpan],
) -> Vec<GraphAudioSourceSpan> {
    spans
        .iter()
        .filter_map(|span| {
            let clip = window.clips.get(span.index)?;
            let occurred_at = audio_timestamp(clip);
            let duration_ms = span.end_ms.saturating_sub(span.start_ms);
            let ended_at =
                occurred_at.map(|at| at + chrono::Duration::milliseconds(i64::from(duration_ms)));
            Some(GraphAudioSourceSpan {
                index: span.index,
                audio_clip_id: clip.id.clone(),
                start_ms: span.start_ms,
                end_ms: span.end_ms,
                occurred_at: occurred_at.map(|at| at.to_rfc3339()),
                ended_at: ended_at.map(|at| at.to_rfc3339()),
                anchor: clip.id == window.anchor_id,
                sensation_id: clip.sensation_id.clone(),
            })
        })
        .collect()
}

fn graph_speech_segments(
    segments: &[SegmentMessage],
    source_started_at: Option<DateTime<Utc>>,
) -> Vec<GraphSpeechSegment> {
    let smallest_segments = segments
        .iter()
        .flat_map(|segment| {
            if segment.words.is_empty() {
                vec![SegmentMessage {
                    text: segment.text.clone(),
                    start_ms: segment.start_ms,
                    end_ms: segment.end_ms,
                    words: Vec::new(),
                }]
            } else {
                segment
                    .words
                    .iter()
                    .map(word_to_segment_message)
                    .collect::<Vec<_>>()
            }
        })
        .collect::<Vec<_>>();

    smallest_segments
        .iter()
        .enumerate()
        .map(|(index, segment)| graph_speech_segment(index, segment, source_started_at))
        .collect()
}

fn word_to_segment_message(word: &WordTiming) -> SegmentMessage {
    SegmentMessage {
        text: word.text.clone(),
        start_ms: word.start_ms,
        end_ms: word.end_ms,
        words: vec![word.clone()],
    }
}

fn graph_speech_segment(
    index: usize,
    segment: &SegmentMessage,
    source_started_at: Option<DateTime<Utc>>,
) -> GraphSpeechSegment {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use psyche::AudioClip;

    fn timestamp() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-05-05T12:34:56Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn graph_clip(id: &str, captured_at: &str) -> GraphAudioClip {
        GraphAudioClip {
            id: id.into(),
            clip: AudioClip {
                mime: "audio/pcm;format=s16le;rate=16000".into(),
                base64: "AAA=".into(),
                sample_rate: 16_000,
                channels: 1,
                transcript: None,
                captured_at: Some(captured_at.into()),
            },
            occurred_at: None,
            sensation_id: Some(format!("sensation:{id}")),
        }
    }

    #[test]
    fn graph_source_spans_link_window_clips_in_order() {
        let window = GraphAudioClipWindow {
            anchor_id: "audio:2".into(),
            clips: vec![
                graph_clip("audio:1", "2026-05-05T12:34:56Z"),
                graph_clip("audio:2", "2026-05-05T12:34:58Z"),
            ],
        };

        let sources = graph_source_spans(
            &window,
            &[
                SourceClipSpan {
                    index: 0,
                    start_ms: 0,
                    end_ms: 1000,
                    sample_count: 16_000,
                },
                SourceClipSpan {
                    index: 1,
                    start_ms: 1000,
                    end_ms: 2500,
                    sample_count: 24_000,
                },
            ],
        );

        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].audio_clip_id, "audio:1");
        assert_eq!(
            sources[0].sensation_id.as_deref(),
            Some("sensation:audio:1")
        );
        assert!(!sources[0].anchor);
        assert_eq!(sources[0].start_ms, 0);
        assert_eq!(sources[0].end_ms, 1000);
        assert_eq!(
            sources[0].ended_at.as_deref(),
            Some("2026-05-05T12:34:57+00:00")
        );
        assert_eq!(sources[1].audio_clip_id, "audio:2");
        assert_eq!(
            sources[1].sensation_id.as_deref(),
            Some("sensation:audio:2")
        );
        assert!(sources[1].anchor);
    }

    #[test]
    fn graph_speech_segments_prefer_word_timings() {
        let segments = graph_speech_segments(
            &[SegmentMessage {
                text: "hello there".into(),
                start_ms: 0,
                end_ms: 800,
                words: vec![
                    WordTiming {
                        text: "hello".into(),
                        start_ms: 0,
                        end_ms: 300,
                    },
                    WordTiming {
                        text: "there".into(),
                        start_ms: 350,
                        end_ms: 800,
                    },
                ],
            }],
            Some(timestamp()),
        );

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].index, 0);
        assert_eq!(segments[0].text, "hello");
        assert_eq!(segments[0].start_ms, 0);
        assert_eq!(segments[0].end_ms, 300);
        assert_eq!(
            segments[0].occurred_at.as_deref(),
            Some("2026-05-05T12:34:56+00:00")
        );
        assert_eq!(segments[1].index, 1);
        assert_eq!(segments[1].text, "there");
        assert_eq!(segments[1].start_ms, 350);
        assert_eq!(segments[1].end_ms, 800);
        assert_eq!(
            segments[1].occurred_at.as_deref(),
            Some("2026-05-05T12:34:56.350+00:00")
        );
    }
}

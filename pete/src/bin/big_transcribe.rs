use std::io::Cursor;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use chrono::{DateTime, Utc};
use clap::Parser;
use dotenvy::dotenv;
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use pete::{
    AsrService, EventBus, HIGH_QUALITY_MULTILINGUAL_MODEL_PATH, SegmentMessage, SourceClipSpan,
    WordTiming, init_logging,
};
use psyche::{
    AudioClip, GraphAudioClip, GraphAudioClipWindow, GraphAudioSourceSpan,
    GraphConsolidatedSpeechCandidate, GraphConsolidatedSpeechSource, GraphSpeechSegment,
    Neo4jClient, audio_clip_id, parse_observed_at,
};
use tokio::time::{MissedTickBehavior, interval};
use tracing::{debug, error, info, warn};

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
    /// Whisper model for aggregate transcription.
    #[arg(long, env = "BIG_TRANSCRIPTION_WHISPER_MODEL")]
    whisper_model: Option<PathBuf>,
    /// Keep original audio clips and first-order transcriptions after consolidation.
    #[arg(long)]
    keep_speech_subnodes: bool,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let (bus, _user_rx) = EventBus::new();
    init_logging(bus.log_sender());
    dotenv().ok();

    let cli = Cli::parse();
    let whisper_model = resolve_whisper_model_path(cli.whisper_model);
    if !whisper_model.exists() {
        anyhow::bail!(
            "Whisper model not found at {}; run `just fetch` or set BIG_TRANSCRIPTION_WHISPER_MODEL",
            whisper_model.display()
        );
    }
    let asr = AsrService::from_whisper_model_path(whisper_model.clone())
        .with_context(|| format!("failed to load Whisper model {}", whisper_model.display()))?;
    let graph = Neo4jClient::new(cli.neo4j_uri, cli.neo4j_user, cli.neo4j_pass);
    let mut ticker = interval(Duration::from_millis(cli.poll_ms.max(100)));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let window_size = cli.window_size.max(cli.min_clips).max(1);
    let min_clips = cli.min_clips.max(1);
    let delete_subnodes = !cli.keep_speech_subnodes;

    info!(
        window_size,
        min_clips, delete_subnodes, "big transcription loop started"
    );
    loop {
        ticker.tick().await;
        if let Err(err) = transcribe_next_window(&graph, &asr, window_size, min_clips).await {
            error!(error = %err, "big transcription loop iteration failed");
        }
        if let Err(err) =
            consolidate_next_big_transcription(&graph, min_clips, delete_subnodes).await
        {
            error!(error = %err, "speech consolidation loop iteration failed");
        }
    }
}

fn resolve_whisper_model_path(cli_model: Option<PathBuf>) -> PathBuf {
    cli_model.unwrap_or_else(|| PathBuf::from(HIGH_QUALITY_MULTILINGUAL_MODEL_PATH))
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

async fn consolidate_next_big_transcription(
    graph: &Neo4jClient,
    min_sources: usize,
    delete_subnodes: bool,
) -> Result<()> {
    let Some(candidate) = graph
        .latest_big_transcription_for_speech_consolidation(min_sources)
        .await
        .context("failed to load speech consolidation candidate")?
    else {
        debug!("no big transcriptions found for speech consolidation");
        return Ok(());
    };

    let fused = fuse_candidate_audio(&candidate).with_context(|| {
        format!(
            "failed to fuse speech transcription {}",
            candidate.transcription_id
        )
    })?;
    let clip_id = audio_clip_id(&fused.clip);
    let report = graph
        .consolidate_big_audio_transcription(
            &candidate,
            &clip_id,
            &fused.clip,
            fused.duration_ms,
            delete_subnodes,
        )
        .await
        .with_context(|| {
            format!(
                "failed to consolidate speech transcription {}",
                candidate.transcription_id
            )
        })?;
    info!(
        transcription_id = %report.transcription_id,
        consolidated_audio_clip_id = %report.consolidated_audio_clip_id,
        source_count = report.source_audio_clip_ids.len(),
        deleted_transcription_count = report.deleted_transcription_ids.len(),
        delete_subnodes,
        "speech transcription consolidated"
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

#[derive(Clone, Debug)]
struct FusedAudio {
    clip: AudioClip,
    duration_ms: u32,
}

fn fuse_candidate_audio(candidate: &GraphConsolidatedSpeechCandidate) -> Result<FusedAudio> {
    anyhow::ensure!(
        !candidate.sources.is_empty(),
        "candidate has no source audio clips"
    );
    let sample_rate = candidate.sources[0].clip.clip.sample_rate;
    anyhow::ensure!(sample_rate > 0, "source audio sample rate must be non-zero");

    let mut samples = Vec::new();
    for source in &candidate.sources {
        append_source_audio(&mut samples, sample_rate, source)?;
    }

    let wav = encode_wav(&samples, sample_rate)?;
    let duration_ms = samples_to_ms(samples.len(), sample_rate);
    Ok(FusedAudio {
        clip: AudioClip {
            mime: "audio/wav".into(),
            base64: BASE64_STANDARD.encode(wav),
            sample_rate,
            channels: 1,
            transcript: Some(candidate.transcript.clone()),
            captured_at: candidate.source_started_at.clone(),
        },
        duration_ms,
    })
}

fn append_source_audio(
    output: &mut Vec<f32>,
    sample_rate: u32,
    source: &GraphConsolidatedSpeechSource,
) -> Result<()> {
    let source_samples = decode_audio_clip_samples(&source.clip.clip, sample_rate)
        .with_context(|| format!("failed to decode source audio clip {}", source.clip.id))?;
    let expected_start = ms_to_samples(source.start_ms, sample_rate);
    if output.len() < expected_start {
        output.resize(expected_start, 0.0);
    } else if output.len() > expected_start {
        warn!(
            source_id = %source.clip.id,
            source_index = source.index,
            expected_start,
            actual_start = output.len(),
            "source span overlaps fused audio; appending at current end"
        );
    }
    output.extend(source_samples);
    let expected_end = ms_to_samples(source.end_ms, sample_rate);
    if output.len() < expected_end {
        output.resize(expected_end, 0.0);
    }
    Ok(())
}

fn decode_audio_clip_samples(clip: &AudioClip, target_sample_rate: u32) -> Result<Vec<f32>> {
    let bytes = BASE64_STANDARD
        .decode(clip.base64.trim().as_bytes())
        .context("failed to decode audio clip base64")?;
    if bytes.is_empty() {
        return Ok(Vec::new());
    }

    let mime = clip.mime.to_ascii_lowercase();
    let (sample_rate, channels, samples) = if mime.starts_with("audio/wav")
        || mime.starts_with("audio/x-wav")
        || bytes.starts_with(b"RIFF")
    {
        decode_wav_samples(&bytes)?
    } else if is_pcm_s16_mime(&mime) {
        (
            clip.sample_rate,
            clip.channels,
            decode_pcm_s16le_samples(&bytes),
        )
    } else {
        return Err(anyhow!("unsupported audio clip MIME type {}", clip.mime));
    };
    anyhow::ensure!(
        sample_rate == target_sample_rate,
        "audio clip sample rate {sample_rate} does not match fused sample rate {target_sample_rate}"
    );
    Ok(downmix_to_mono(samples, channels))
}

fn decode_wav_samples(bytes: &[u8]) -> Result<(u32, u16, Vec<f32>)> {
    let mut reader =
        WavReader::new(Cursor::new(bytes)).context("failed to read audio clip WAV data")?;
    let spec = reader.spec();
    let samples = match spec.sample_format {
        SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to decode float WAV samples")?,
        SampleFormat::Int if spec.bits_per_sample <= 16 => reader
            .samples::<i16>()
            .map(|sample| sample.map(|sample| sample as f32 / i16::MAX as f32))
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to decode 16-bit WAV samples")?,
        SampleFormat::Int if spec.bits_per_sample <= 32 => reader
            .samples::<i32>()
            .map(|sample| sample.map(|sample| sample as f32 / i32::MAX as f32))
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to decode 32-bit WAV samples")?,
        _ => {
            return Err(anyhow!(
                "unsupported WAV bit depth {}",
                spec.bits_per_sample
            ));
        }
    };
    Ok((spec.sample_rate, spec.channels, samples))
}

fn decode_pcm_s16le_samples(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|chunk| {
            let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
            sample as f32 / i16::MAX as f32
        })
        .collect()
}

fn downmix_to_mono(samples: Vec<f32>, channels: u16) -> Vec<f32> {
    if channels <= 1 {
        return samples;
    }
    let channels = usize::from(channels);
    samples
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

fn is_pcm_s16_mime(mime: &str) -> bool {
    mime.starts_with("audio/pcm") || mime.starts_with("audio/l16") || mime.contains("format=s16")
}

fn encode_wav(samples: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let spec = WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut writer = WavWriter::new(&mut cursor, spec)?;
        for &sample in samples {
            let clamped = sample.clamp(-1.0, 1.0);
            writer.write_sample((clamped * i16::MAX as f32) as i16)?;
        }
        writer.finalize()?;
    }
    Ok(cursor.into_inner())
}

fn ms_to_samples(ms: u32, sample_rate: u32) -> usize {
    if sample_rate == 0 {
        return 0;
    }
    ((u128::from(ms) * u128::from(sample_rate)) / 1000).min(usize::MAX as u128) as usize
}

fn samples_to_ms(samples: usize, sample_rate: u32) -> u32 {
    if sample_rate == 0 {
        return 0;
    }
    let ms = (samples as u128).saturating_mul(1000) / u128::from(sample_rate);
    ms.min(u128::from(u32::MAX)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn pcm_source(
        id: &str,
        index: usize,
        samples: &[i16],
        start_ms: u32,
        end_ms: u32,
    ) -> GraphConsolidatedSpeechSource {
        let bytes = samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect::<Vec<_>>();
        GraphConsolidatedSpeechSource {
            index,
            clip: GraphAudioClip {
                id: id.into(),
                clip: AudioClip {
                    mime: "audio/pcm;format=s16le;rate=16000".into(),
                    base64: BASE64_STANDARD.encode(bytes),
                    sample_rate: 16_000,
                    channels: 1,
                    transcript: None,
                    captured_at: Some("2026-05-05T12:34:56Z".into()),
                },
                occurred_at: None,
                sensation_id: None,
            },
            start_ms,
            end_ms,
            transcription_ids: Vec::new(),
        }
    }

    #[test]
    fn resolves_big_transcription_model_to_large_multilingual_default() {
        let model = resolve_whisper_model_path(None);

        assert_eq!(model, PathBuf::from(HIGH_QUALITY_MULTILINGUAL_MODEL_PATH));
    }

    #[test]
    fn resolves_big_transcription_model_from_specific_override() {
        let model = resolve_whisper_model_path(Some(PathBuf::from("models/whisper/custom.bin")));

        assert_eq!(model, PathBuf::from("models/whisper/custom.bin"));
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

    #[test]
    fn fuse_candidate_audio_stitches_sources_with_span_gaps() {
        let candidate = GraphConsolidatedSpeechCandidate {
            transcription_id: "big:1".into(),
            transcript: "hello there".into(),
            source_started_at: Some("2026-05-05T12:34:56Z".into()),
            source_ended_at: None,
            sources: vec![
                pcm_source("audio:1", 0, &[1000, -1000], 0, 2),
                pcm_source("audio:2", 1, &[2000, -2000], 4, 6),
            ],
        };

        let fused = fuse_candidate_audio(&candidate).unwrap();
        let wav = BASE64_STANDARD
            .decode(fused.clip.base64.as_bytes())
            .unwrap();
        let (sample_rate, channels, samples) = decode_wav_samples(&wav).unwrap();

        assert_eq!(sample_rate, 16_000);
        assert_eq!(channels, 1);
        assert_eq!(fused.clip.mime, "audio/wav");
        assert_eq!(fused.clip.transcript.as_deref(), Some("hello there"));
        assert!(samples.len() >= ms_to_samples(6, 16_000));
    }

    #[test]
    fn decode_audio_clip_samples_downmixes_stereo_pcm() {
        let samples = [1000i16, 3000, -1000, -3000];
        let bytes = samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect::<Vec<_>>();
        let clip = AudioClip {
            mime: "audio/pcm;format=s16le;rate=16000".into(),
            base64: BASE64_STANDARD.encode(bytes),
            sample_rate: 16_000,
            channels: 2,
            transcript: None,
            captured_at: None,
        };

        let decoded = decode_audio_clip_samples(&clip, 16_000).unwrap();

        assert_eq!(decoded.len(), 2);
        assert!(decoded[0] > 0.0);
        assert!(decoded[1] < 0.0);
    }
}

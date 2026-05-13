use std::{env, process::Command, time::Duration};

use anyhow::Context;
use chrono::{DateTime, Utc};
use clap::Parser;
use dotenvy::dotenv;
use pete::{AsrService, EventBus, SegmentMessage, WordTiming, init_logging};
use psyche::{GraphAudioClip, GraphSpeechSegment, Neo4jClient, parse_observed_at};
use tokio::time::{MissedTickBehavior, interval};
use tracing::{error, info, trace};

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
    ensure_whisper_gpu_enabled()?;

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

fn ensure_whisper_gpu_enabled() -> anyhow::Result<()> {
    validate_asr_use_gpu(env::var("ASR_USE_GPU").ok().as_deref())?;
    ensure_cuda_runtime_supported()
}

fn validate_asr_use_gpu(value: Option<&str>) -> anyhow::Result<()> {
    let use_gpu = value
        .unwrap_or("true")
        .parse::<bool>()
        .context("invalid ASR_USE_GPU")?;
    anyhow::ensure!(
        use_gpu,
        "transcription requires Whisper GPU acceleration; unset ASR_USE_GPU or set ASR_USE_GPU=true"
    );
    Ok(())
}

fn ensure_cuda_runtime_supported() -> anyhow::Result<()> {
    let driver_cuda = nvidia_driver_cuda_version()?;
    let runtime_cuda = cuda_runtime_version()?;
    anyhow::ensure!(
        runtime_cuda <= driver_cuda,
        "transcription requires CUDA GPU execution, but CUDA runtime {runtime_cuda} is newer than the NVIDIA driver supports ({driver_cuda}); upgrade the NVIDIA driver or use a CUDA {driver_cuda}-compatible toolkit/runtime"
    );
    Ok(())
}

fn nvidia_driver_cuda_version() -> anyhow::Result<CudaVersion> {
    let output = command_stdout(&mut Command::new("nvidia-smi"))
        .context("failed to run nvidia-smi; transcription requires an NVIDIA GPU")?;
    parse_nvidia_smi_cuda_version(&output)
        .context("failed to read supported CUDA version from nvidia-smi")
}

fn cuda_runtime_version() -> anyhow::Result<CudaVersion> {
    let output = command_stdout(Command::new("nvcc").arg("--version"))
        .context("failed to run nvcc; transcription requires the CUDA toolkit/runtime")?;
    parse_nvcc_cuda_version(&output).context("failed to read CUDA runtime version from nvcc")
}

fn command_stdout(command: &mut Command) -> anyhow::Result<String> {
    let output = command.output()?;
    anyhow::ensure!(
        output.status.success(),
        "command exited with status {}",
        output.status
    );
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CudaVersion {
    major: u32,
    minor: u32,
}

impl std::fmt::Display for CudaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

fn parse_nvidia_smi_cuda_version(output: &str) -> Option<CudaVersion> {
    output
        .split_once("CUDA Version:")?
        .1
        .split_whitespace()
        .find_map(parse_cuda_version)
}

fn parse_nvcc_cuda_version(output: &str) -> Option<CudaVersion> {
    output
        .split_once("release ")?
        .1
        .split(|ch: char| ch.is_whitespace() || ch == ',')
        .find_map(parse_cuda_version)
}

fn parse_cuda_version(value: &str) -> Option<CudaVersion> {
    let value = value.trim_matches(|ch: char| !ch.is_ascii_digit() && ch != '.');
    let (major, minor) = value.split_once('.')?;
    Some(CudaVersion {
        major: major.parse().ok()?,
        minor: minor.parse().ok()?,
    })
}

async fn transcribe_next_clip(graph: &Neo4jClient, asr: &AsrService) -> anyhow::Result<()> {
    let Some(audio) = graph
        .latest_untranscribed_audio_clip()
        .await
        .context("failed to load latest untranscribed audio clip")?
    else {
        trace!("no untranscribed audio clips found");
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
            audio.sensation_id.as_deref(),
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

    fn timestamp() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-05-05T12:34:56Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn transcription_allows_default_gpu_setting() {
        validate_asr_use_gpu(None).unwrap();
        validate_asr_use_gpu(Some("true")).unwrap();
    }

    #[test]
    fn transcription_rejects_disabled_gpu_setting() {
        let err = validate_asr_use_gpu(Some("false")).unwrap_err();
        assert!(
            err.to_string()
                .contains("transcription requires Whisper GPU acceleration")
        );
    }

    #[test]
    fn parses_nvidia_smi_cuda_version() {
        let output = "| NVIDIA-SMI 570.195.03 Driver Version: 570.195.03 CUDA Version: 12.8 |";
        assert_eq!(
            parse_nvidia_smi_cuda_version(output),
            Some(CudaVersion {
                major: 12,
                minor: 8
            })
        );
    }

    #[test]
    fn parses_nvcc_cuda_version() {
        let output = "Cuda compilation tools, release 13.2, V13.2.78";
        assert_eq!(
            parse_nvcc_cuda_version(output),
            Some(CudaVersion {
                major: 13,
                minor: 2
            })
        );
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
    fn graph_speech_segments_fall_back_to_whisper_segments() {
        let segments = graph_speech_segments(
            &[SegmentMessage {
                text: "hello there".into(),
                start_ms: 250,
                end_ms: 1250,
                words: Vec::new(),
            }],
            Some(timestamp()),
        );

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "hello there");
        assert_eq!(segments[0].start_ms, 250);
        assert_eq!(segments[0].end_ms, 1250);
        assert_eq!(
            segments[0].occurred_at.as_deref(),
            Some("2026-05-05T12:34:56.250+00:00")
        );
    }
}

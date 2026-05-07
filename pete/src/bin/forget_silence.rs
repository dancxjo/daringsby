use std::io::Cursor;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use chrono::Utc;
use clap::Parser;
use dotenvy::dotenv;
use hound::{SampleFormat, WavReader};
use pete::{EventBus, init_logging};
use reqwest::Url;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::time::{MissedTickBehavior, interval};
use tracing::{debug, error, info, warn};

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Forget silent AudioClip graph nodes while preserving their Sensation nodes"
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
    #[arg(long, env = "FORGET_SILENCE_POLL_MS", default_value_t = 1000)]
    poll_ms: u64,
    /// Number of unchecked AudioClip nodes to inspect per loop.
    #[arg(long, env = "FORGET_SILENCE_BATCH_SIZE", default_value_t = 100)]
    batch_size: usize,
    /// RMS threshold at or below which an audio window is treated as silence.
    #[arg(long, env = "FORGET_SILENCE_THRESHOLD", default_value_t = 0.015)]
    silence_threshold: f32,
    /// RMS window size for speech detection.
    #[arg(long, env = "FORGET_SILENCE_WINDOW_MS", default_value_t = 20)]
    window_ms: u64,
    /// Print decisions without deleting or marking graph nodes.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Clone)]
struct Neo4jHttp {
    endpoint: Url,
    user: String,
    pass: String,
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct AudioCandidate {
    id: String,
    mime: Option<String>,
    base64: String,
    sample_rate: Option<u32>,
    channels: Option<u16>,
    transcript: Option<String>,
}

#[derive(Debug)]
struct AudioStats {
    duration_ms: u64,
    rms: f32,
    peak: f32,
    silent: bool,
}

#[derive(Debug, Deserialize)]
struct DeleteCounts {
    audio: u64,
    transcripts: u64,
    segments: u64,
    voice_runs: u64,
    voice_samples: u64,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let (bus, _user_rx) = EventBus::new();
    init_logging(bus.log_sender());
    dotenv().ok();

    let cli = Cli::parse();
    let graph = Neo4jHttp::new(
        cli.neo4j_uri.clone(),
        cli.neo4j_user.clone(),
        cli.neo4j_pass.clone(),
    )?;
    let mut ticker = interval(Duration::from_millis(cli.poll_ms.max(100)));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    info!(
        batch_size = cli.batch_size,
        threshold = cli.silence_threshold,
        window_ms = cli.window_ms,
        dry_run = cli.dry_run,
        "forget-silence loop started"
    );

    loop {
        ticker.tick().await;
        if let Err(err) = sweep_once(&graph, &cli).await {
            error!(error = %err, "forget-silence loop iteration failed");
        }
    }
}

async fn sweep_once(graph: &Neo4jHttp, cli: &Cli) -> Result<()> {
    let candidates = graph.fetch_candidates(cli.batch_size.max(1)).await?;
    if candidates.is_empty() {
        debug!("no unchecked audio clips found");
        return Ok(());
    }

    let mut silent_ids = Vec::new();
    let mut checked = Vec::new();
    for candidate in candidates {
        match classify_candidate(&candidate, cli.silence_threshold, cli.window_ms) {
            Ok(stats) => {
                info!(
                    clip_id = %candidate.id,
                    duration_ms = stats.duration_ms,
                    rms = stats.rms,
                    peak = stats.peak,
                    transcript = candidate.transcript.as_deref().unwrap_or(""),
                    silent = stats.silent,
                    "checked audio clip for silence"
                );
                if stats.silent {
                    silent_ids.push(candidate.id.clone());
                } else {
                    checked.push(json!({
                        "id": candidate.id,
                        "rms": stats.rms,
                        "peak": stats.peak,
                        "duration_ms": stats.duration_ms,
                    }));
                }
            }
            Err(err) => {
                warn!(clip_id = %candidate.id, error = %err, "failed to classify audio clip");
                checked.push(json!({
                    "id": candidate.id,
                    "error": err.to_string(),
                }));
            }
        }
    }

    if cli.dry_run {
        if !silent_ids.is_empty() {
            println!("would forget silent clips: {}", silent_ids.join(", "));
        }
        return Ok(());
    }

    if !checked.is_empty() {
        graph.mark_checked(&checked).await?;
    }
    if !silent_ids.is_empty() {
        let counts = graph.delete_silent_audio(&silent_ids).await?;
        info!(
            audio = counts.audio,
            transcripts = counts.transcripts,
            segments = counts.segments,
            voice_runs = counts.voice_runs,
            voice_samples = counts.voice_samples,
            "forgot silent audio clips"
        );
    }

    Ok(())
}

fn classify_candidate(
    candidate: &AudioCandidate,
    threshold: f32,
    window_ms: u64,
) -> Result<AudioStats> {
    let decoded = BASE64_STANDARD
        .decode(candidate.base64.as_bytes())
        .with_context(|| format!("failed to decode base64 for {}", candidate.id))?;
    let mime = candidate
        .mime
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let (samples, sample_rate, channels) = if mime.starts_with("audio/wav")
        || mime.starts_with("audio/x-wav")
        || decoded.starts_with(b"RIFF")
    {
        decode_wav(&decoded)?
    } else {
        decode_pcm_s16(
            &decoded,
            candidate.sample_rate.unwrap_or(16_000),
            candidate.channels.unwrap_or(1),
        )
    };

    let sample_count = samples.len();
    let channel_count = usize::from(channels.max(1));
    let duration_ms = ((sample_count as u128).saturating_mul(1000)
        / u128::from(sample_rate.max(1))
        / channel_count as u128)
        .min(u128::from(u64::MAX)) as u64;
    let rms = rms(&samples);
    let peak = samples
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0, f32::max);
    let silent =
        !contains_audio_above_threshold(&samples, sample_rate, channels, threshold, window_ms);

    Ok(AudioStats {
        duration_ms,
        rms,
        peak,
        silent,
    })
}

fn decode_pcm_s16(bytes: &[u8], sample_rate: u32, channels: u16) -> (Vec<f32>, u32, u16) {
    let samples = bytes
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / i16::MAX as f32)
        .collect();
    (samples, sample_rate, channels)
}

fn decode_wav(bytes: &[u8]) -> Result<(Vec<f32>, u32, u16)> {
    let mut reader = WavReader::new(Cursor::new(bytes)).context("failed to read WAV audio")?;
    let spec = reader.spec();
    let samples = match spec.sample_format {
        SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to read float WAV samples")?,
        SampleFormat::Int if spec.bits_per_sample <= 16 => reader
            .samples::<i16>()
            .map(|sample| sample.map(|sample| sample as f32 / i16::MAX as f32))
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to read 16-bit WAV samples")?,
        SampleFormat::Int => {
            let max = ((1_i64 << (u32::from(spec.bits_per_sample).saturating_sub(1))) - 1) as f32;
            reader
                .samples::<i32>()
                .map(|sample| sample.map(|sample| sample as f32 / max))
                .collect::<std::result::Result<Vec<_>, _>>()
                .context("failed to read integer WAV samples")?
        }
    };
    Ok((samples, spec.sample_rate, spec.channels))
}

fn contains_audio_above_threshold(
    samples: &[f32],
    sample_rate: u32,
    channels: u16,
    threshold: f32,
    window_ms: u64,
) -> bool {
    if samples.is_empty() {
        return false;
    }
    let channels = usize::from(channels.max(1));
    let window =
        ((u128::from(sample_rate.max(1)) * channels as u128 * u128::from(window_ms.max(1))) / 1000)
            .max(1)
            .min(usize::MAX as u128) as usize;
    if samples.len() <= window {
        return rms(samples) > threshold;
    }
    let step = (window / 2).max(1);
    let mut start = 0;
    while start + window <= samples.len() {
        if rms(&samples[start..start + window]) > threshold {
            return true;
        }
        start += step;
    }
    false
}

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq = samples
        .iter()
        .map(|sample| {
            let value = f64::from(*sample);
            value * value
        })
        .sum::<f64>();
    (sum_sq / samples.len() as f64).sqrt() as f32
}

impl Neo4jHttp {
    fn new(uri: String, user: String, pass: String) -> Result<Self> {
        Ok(Self {
            endpoint: neo4j_http_endpoint(&uri)?,
            user,
            pass,
            client: reqwest::Client::new(),
        })
    }

    async fn fetch_candidates(&self, limit: usize) -> Result<Vec<AudioCandidate>> {
        let rows = self
            .query_rows(
                r#"
                    MATCH (a:GraphNode:AudioClip)
                    WHERE a.base64 IS NOT NULL
                      AND a.silence_checked_at IS NULL
                      AND a.silence_check_error IS NULL
                    RETURN {
                        id: a.id,
                        mime: a.mime,
                        base64: a.base64,
                        sample_rate: a.sample_rate,
                        channels: a.channels,
                        transcript: a.transcript
                    } AS audio
                    ORDER BY coalesce(a.captured_at, a.occurred_at, a.id)
                    LIMIT $limit
                "#,
                json!({ "limit": limit as i64 }),
                "fetching unchecked audio clips",
            )
            .await?;

        rows.into_iter()
            .map(|row| {
                let value = row
                    .first()
                    .cloned()
                    .ok_or_else(|| anyhow!("Neo4j row did not contain audio candidate"))?;
                serde_json::from_value(value).context("failed to decode audio candidate row")
            })
            .collect()
    }

    async fn mark_checked(&self, clips: &[Value]) -> Result<()> {
        self.commit(
            r#"
                UNWIND $clips AS clip
                MATCH (a:GraphNode:AudioClip {id: clip.id})
                SET a.silence_checked_at = $checked_at,
                    a.silence_rms = clip.rms,
                    a.silence_peak = clip.peak,
                    a.silence_duration_ms = clip.duration_ms,
                    a.silence_check_error = clip.error
            "#,
            json!({
                "clips": clips,
                "checked_at": Utc::now().to_rfc3339(),
            }),
            "marking non-silent audio clips checked",
        )
        .await
    }

    async fn delete_silent_audio(&self, ids: &[String]) -> Result<DeleteCounts> {
        let rows = self
            .query_rows(
                r#"
                    MATCH (a:GraphNode:AudioClip)
                    WHERE a.id IN $ids
                    WITH collect(DISTINCT a) AS audio, $ids AS ids
                    CALL {
                        WITH ids
                        MATCH (source:GraphNode:AudioClip)-[:HAS_TRANSCRIPTION|HAS_BIG_TRANSCRIPTION]->(t:GraphNode:Transcription)
                        WHERE source.id IN ids
                        OPTIONAL MATCH (other:GraphNode:AudioClip)-[:HAS_TRANSCRIPTION|HAS_BIG_TRANSCRIPTION]->(t)
                        WHERE NOT other.id IN ids
                        WITH t, count(DISTINCT other) AS other_count
                        WHERE other_count = 0
                        OPTIONAL MATCH (t)-[:HAS_SEGMENT]->(segment:GraphNode:SpeechSegment)
                        RETURN collect(DISTINCT t) AS transcripts,
                               collect(DISTINCT segment) AS segments
                    }
                    CALL {
                        WITH ids
                        OPTIONAL MATCH (run:GraphNode:VoiceRecognitionRun)
                        WHERE run.audio_clip_id IN ids
                        OPTIONAL MATCH (run)-[:PRODUCED_SAMPLE]->(sample:GraphNode:VoiceSample)
                        RETURN collect(DISTINCT run) AS voice_runs,
                               collect(DISTINCT sample) AS voice_samples
                    }
                    WITH audio, transcripts, segments, voice_runs, voice_samples,
                         size(audio) AS audio_count,
                         size(transcripts) AS transcript_count,
                         size(segments) AS segment_count,
                         size(voice_runs) AS voice_run_count,
                         size(voice_samples) AS voice_sample_count
                    FOREACH (node IN audio + transcripts + segments + voice_runs + voice_samples |
                        DETACH DELETE node
                    )
                    RETURN {
                        audio: audio_count,
                        transcripts: transcript_count,
                        segments: segment_count,
                        voice_runs: voice_run_count,
                        voice_samples: voice_sample_count
                    } AS counts
                "#,
                json!({ "ids": ids }),
                "forgetting silent audio clips",
            )
            .await?;
        let value = rows
            .first()
            .and_then(|row| row.first())
            .cloned()
            .ok_or_else(|| anyhow!("Neo4j did not return delete counts"))?;
        serde_json::from_value(value).context("failed to decode delete counts")
    }

    async fn query_rows(
        &self,
        statement: &str,
        parameters: Value,
        action: &str,
    ) -> Result<Vec<Vec<Value>>> {
        let body = json!({
            "statements": [{
                "statement": statement,
                "parameters": parameters,
                "resultDataContents": ["row"],
            }]
        });
        let response = self
            .client
            .post(self.endpoint.clone())
            .basic_auth(&self.user, Some(&self.pass))
            .json(&body)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .with_context(|| format!("failed while {action} at {}", self.endpoint))?;
        self.decode_rows_response(response, action).await
    }

    async fn commit(&self, statement: &str, parameters: Value, action: &str) -> Result<()> {
        let body = json!({
            "statements": [{
                "statement": statement,
                "parameters": parameters,
            }]
        });
        let response = self
            .client
            .post(self.endpoint.clone())
            .basic_auth(&self.user, Some(&self.pass))
            .json(&body)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .with_context(|| format!("failed while {action} at {}", self.endpoint))?;
        self.decode_commit_response(response, action).await
    }

    async fn decode_rows_response(
        &self,
        response: reqwest::Response,
        action: &str,
    ) -> Result<Vec<Vec<Value>>> {
        if !response.status().is_success() {
            bail!("Neo4j HTTP {} while {action}", response.status());
        }
        let body: Value = response
            .json()
            .await
            .with_context(|| format!("failed to decode Neo4j response while {action}"))?;
        ensure_no_neo4j_errors(&body, action)?;
        Ok(body
            .pointer("/results/0/data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|entry| entry.get("row").and_then(Value::as_array).cloned())
            .collect())
    }

    async fn decode_commit_response(
        &self,
        response: reqwest::Response,
        action: &str,
    ) -> Result<()> {
        if !response.status().is_success() {
            bail!("Neo4j HTTP {} while {action}", response.status());
        }
        let body: Value = response
            .json()
            .await
            .with_context(|| format!("failed to decode Neo4j response while {action}"))?;
        ensure_no_neo4j_errors(&body, action)
    }
}

fn ensure_no_neo4j_errors(body: &Value, action: &str) -> Result<()> {
    let errors = body
        .get("errors")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !errors.is_empty() {
        bail!("Neo4j returned errors while {action}: {errors:?}");
    }
    Ok(())
}

fn neo4j_http_endpoint(uri: &str) -> Result<Url> {
    let parsed = Url::parse(uri).with_context(|| format!("invalid Neo4j URI {uri}"))?;
    let mut url = match parsed.scheme() {
        "http" | "https" => parsed,
        "bolt" | "neo4j" => convert_neo4j_url(&parsed, "http", 7474)?,
        "bolt+s" | "neo4j+s" => convert_neo4j_url(&parsed, "https", 7473)?,
        scheme => bail!("unsupported Neo4j URI scheme {scheme}"),
    };
    url.set_path("/db/neo4j/tx/commit");
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

fn convert_neo4j_url(source: &Url, scheme: &str, default_port: u16) -> Result<Url> {
    let host = source
        .host_str()
        .with_context(|| format!("Neo4j URI {} is missing a host", source.as_str()))?;
    let port = match source.port() {
        Some(7687) | None => default_port,
        Some(port) => port,
    };
    Url::parse(&format!("{scheme}://{host}:{port}"))
        .with_context(|| format!("failed to convert Neo4j URI {}", source.as_str()))
}

#[cfg(test)]
mod tests {
    use super::{contains_audio_above_threshold, rms};

    #[test]
    fn short_quiet_clip_is_silence() {
        let samples = vec![0.001; 320];

        assert!(!contains_audio_above_threshold(
            &samples, 16_000, 1, 0.015, 20
        ));
    }

    #[test]
    fn window_above_threshold_is_not_silence() {
        let mut samples = vec![0.001; 640];
        samples[320..640].fill(0.05);

        assert!(contains_audio_above_threshold(
            &samples, 16_000, 1, 0.015, 20
        ));
    }

    #[test]
    fn rms_handles_empty_audio() {
        assert_eq!(rms(&[]), 0.0);
    }
}

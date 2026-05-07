use std::{f32::consts::PI, io::Cursor, path::PathBuf, time::Duration};

use anyhow::{Context, anyhow, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use chrono::{DateTime, Utc};
use clap::Parser;
use dotenvy::dotenv;
use hound::{SampleFormat, WavReader};
use pete::{EventBus, init_logging};
use psyche::{
    GraphVoiceClip, GraphVoiceMatch, GraphVoiceRecognition, GraphVoiceSample, GraphVoiceSignature,
    Neo4jClient, QdrantClient, parse_observed_at,
};
use tokio::time::{MissedTickBehavior, interval};
use tracing::{error, info, trace, warn};

const DEFAULT_VOICE_EMBEDDING_MODEL_PATH: &str = "models/voice/speaker_embedding_extractor.onnx";
const ANALYSIS_SAMPLE_RATE: u32 = 16_000;
const MIN_VOICE_EMBEDDING_SAMPLES_22050: usize = 1024;

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Recognize voices in stored AudioClip graph nodes and link the results"
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
    /// Qdrant HTTP endpoint.
    #[arg(long, env = "QDRANT_URL", default_value = "http://localhost:6333")]
    qdrant_url: String,
    /// Voice embedding ONNX model path.
    #[arg(long, env = "VOICE_EMBEDDING_MODEL")]
    model: Option<String>,
    /// Delay between graph polling attempts.
    #[arg(long, env = "VRECOG_POLL_MS", default_value_t = 1000)]
    poll_ms: u64,
    /// Minimum Qdrant similarity for treating a detected voice as a known voice.
    #[arg(long, env = "VRECOG_VOICE_MATCH_THRESHOLD", default_value_t = 0.86)]
    voice_match_threshold: f32,
    /// Process at most one clip and exit.
    #[arg(long)]
    once: bool,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let (bus, _user_rx) = EventBus::new();
    init_logging(bus.log_sender());
    dotenv().ok();

    let cli = Cli::parse();
    let model_path = cli
        .model
        .or_else(default_voice_embedding_model_path)
        .ok_or_else(|| {
            anyhow!(
                "voice embedding model not configured; set VOICE_EMBEDDING_MODEL or run `just fetch`"
            )
        })?;
    let graph = Neo4jClient::new(cli.neo4j_uri, cli.neo4j_user, cli.neo4j_pass);
    let qdrant = QdrantClient::new(cli.qdrant_url);
    let mut recognizer = VoiceRecognizer::new(model_path)?;

    if cli.once {
        process_next_clip(&graph, &qdrant, &mut recognizer, cli.voice_match_threshold).await?;
        return Ok(());
    }

    let mut ticker = interval(Duration::from_millis(cli.poll_ms.max(100)));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    info!("voice recognition loop started");
    loop {
        ticker.tick().await;
        if let Err(err) =
            process_next_clip(&graph, &qdrant, &mut recognizer, cli.voice_match_threshold).await
        {
            error!(error = %err, "voice recognition loop iteration failed");
        }
    }
}

async fn process_next_clip(
    graph: &Neo4jClient,
    qdrant: &QdrantClient,
    recognizer: &mut VoiceRecognizer,
    voice_match_threshold: f32,
) -> anyhow::Result<()> {
    let Some(clip) = graph
        .latest_unprocessed_audio_clip_for_voice_recognition()
        .await
        .context("failed to load latest unprocessed audio clip")?
    else {
        trace!("no unprocessed audio clips found");
        return Ok(());
    };

    info!(clip_id = %clip.id, "recognizing voice in audio clip");
    match recognizer
        .recognize(&clip, graph, qdrant, voice_match_threshold)
        .await
        .with_context(|| format!("failed to recognize voice in audio clip {}", clip.id))?
    {
        VoiceRecognitionOutcome::Recognized(recognition) => {
            graph
                .attach_voice_recognition(&clip, &recognizer.model, &recognition)
                .await
                .with_context(|| {
                    format!(
                        "failed to attach voice recognition for audio clip {}",
                        clip.id
                    )
                })?;
            info!(
                clip_id = %clip.id,
                user_id = %recognition.signature.user_id,
                vector_id = %recognition.vector_id,
                "attached voice recognition"
            );
        }
        VoiceRecognitionOutcome::Skipped(reason) => {
            graph
                .attach_skipped_voice_recognition(&clip, &recognizer.model, &reason)
                .await
                .with_context(|| {
                    format!(
                        "failed to attach skipped voice recognition for audio clip {}",
                        clip.id
                    )
                })?;
            warn!(clip_id = %clip.id, %reason, "skipped voice recognition for audio clip");
        }
    }
    Ok(())
}

enum VoiceRecognitionOutcome {
    Recognized(GraphVoiceRecognition),
    Skipped(String),
}

struct VoiceRecognizer {
    extractor: voxudio::SpeakerEmbeddingExtractor,
    model: String,
}

impl VoiceRecognizer {
    fn new(model: String) -> anyhow::Result<Self> {
        let extractor = voxudio::SpeakerEmbeddingExtractor::new(&model)
            .with_context(|| format!("failed to load voice embedding model {model}"))?;
        info!(%model, "voice embedding model loaded");
        Ok(Self { extractor, model })
    }

    async fn recognize(
        &mut self,
        clip: &GraphVoiceClip,
        graph: &Neo4jClient,
        qdrant: &QdrantClient,
        voice_match_threshold: f32,
    ) -> anyhow::Result<VoiceRecognitionOutcome> {
        let samples = match decode_audio_clip_samples(&clip.clip, ANALYSIS_SAMPLE_RATE) {
            Ok(samples) => samples,
            Err(err) => {
                return Ok(VoiceRecognitionOutcome::Skipped(format!(
                    "failed to decode audio clip: {err}"
                )));
            }
        };
        if samples.is_empty() {
            return Ok(VoiceRecognitionOutcome::Skipped(
                "audio clip had no samples".into(),
            ));
        }
        let audio_22050 = voxudio::resample::<16000, 22050, f32>(&samples, 1, 1)
            .context("failed to resample audio for voice embedding")?;
        if audio_22050.len() < MIN_VOICE_EMBEDDING_SAMPLES_22050 {
            return Ok(VoiceRecognitionOutcome::Skipped(format!(
                "audio clip too short for voice embedding model: {} samples at 22050 Hz",
                audio_22050.len()
            )));
        }
        let embeddings = match self.extractor.extract(&audio_22050, 1).await {
            Ok(embeddings) => embeddings,
            Err(err) if is_short_audio_embedding_error(&err) => {
                return Ok(VoiceRecognitionOutcome::Skipped(format!(
                    "audio clip too short for voice embedding model: {err}"
                )));
            }
            Err(err) => return Err(err).context("failed to extract voice embedding"),
        };
        let embedding = embeddings
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("voice embedding model returned no embeddings"))?
            .to_vec();
        let user_id = voice_id_from_embedding(&embedding);
        let vector_id = qdrant
            .store_voice_vector_for_sensation(
                Some(&clip.id),
                clip.sensation_id.as_deref(),
                Some(&user_id),
                &embedding,
            )
            .await
            .context("failed to store voice vector")?
            .to_string();
        let recognition = match_voice(
            graph,
            qdrant,
            &embedding,
            &vector_id,
            voice_match_threshold,
            &clip.id,
        )
        .await?;
        let timestamp = audio_timestamp(clip).unwrap_or_else(Utc::now);
        let features = analyze_voice(&samples, ANALYSIS_SAMPLE_RATE);
        let signature = GraphVoiceSignature {
            user_id: user_id.clone(),
            fundamental_frequency: features.fundamental_frequency,
            frequency_range: features.frequency_range,
            formant_frequencies: features.formant_frequencies.clone(),
            speech_rate: features.speech_rate,
            mfcc_signature: features.mfcc.clone(),
            spectral_centroid: features.spectral_centroid,
            jitter: features.jitter,
            shimmer: features.shimmer,
            harmonic_to_noise_ratio: features.harmonic_to_noise_ratio,
            sample_count: 1,
            last_updated: timestamp,
            tags: vec!["voice".into(), "voxudio".into()],
        };
        let sample = GraphVoiceSample {
            id: format!("voice-sample:{}", clip.id),
            user_id,
            duration_ms: duration_ms(samples.len(), ANALYSIS_SAMPLE_RATE),
            sample_rate: ANALYSIS_SAMPLE_RATE,
            fundamental_frequency: features.fundamental_frequency,
            formant_frequencies: features.formant_frequencies,
            mfcc: features.mfcc,
            quality_score: features.quality_score,
            timestamp,
        };
        Ok(VoiceRecognitionOutcome::Recognized(GraphVoiceRecognition {
            signature,
            sample,
            vector_id,
            embedding_len: embedding.len(),
            recognition,
        }))
    }
}

async fn match_voice(
    graph: &Neo4jClient,
    qdrant: &QdrantClient,
    embedding: &[f32],
    vector_id: &str,
    threshold: f32,
    clip_id: &str,
) -> anyhow::Result<Option<GraphVoiceMatch>> {
    let Some(neighbor) = qdrant
        .nearest_voice_neighbor(embedding, vector_id, threshold)
        .await
        .with_context(|| format!("failed to search nearest voice neighbor for {clip_id}"))?
    else {
        return Ok(None);
    };
    let Some(identity) = graph
        .voice_identity_for_vector_neighbor(&neighbor.point_id)
        .await
        .with_context(|| {
            format!(
                "failed to load voice identity for nearest vector {}",
                neighbor.point_id
            )
        })?
    else {
        return Ok(None);
    };
    Ok(Some(GraphVoiceMatch {
        voice_id: identity.voice_id,
        identity: identity.identity,
        nearest_vector_id: neighbor.point_id,
        score: neighbor.score,
    }))
}

fn is_short_audio_embedding_error(error: &impl std::fmt::Display) -> bool {
    let message = error.to_string();
    message.contains("window_size <= signal_size") || message.contains("dft size is smaller")
}

#[derive(Debug)]
struct VoiceFeatures {
    fundamental_frequency: f32,
    frequency_range: (f32, f32),
    formant_frequencies: Vec<f32>,
    speech_rate: f32,
    mfcc: Vec<f32>,
    spectral_centroid: f32,
    jitter: f32,
    shimmer: f32,
    harmonic_to_noise_ratio: f32,
    quality_score: f32,
}

fn analyze_voice(samples: &[f32], sample_rate: u32) -> VoiceFeatures {
    let frames = voiced_frames(samples, sample_rate);
    let f0_values = frames
        .iter()
        .filter_map(|frame| estimate_pitch(frame, sample_rate).map(|(pitch, _)| pitch))
        .collect::<Vec<_>>();
    let rms_values = frames.iter().map(|frame| rms(frame)).collect::<Vec<_>>();
    let fundamental_frequency = mean_or(&f0_values, 0.0);
    let frequency_range = if f0_values.is_empty() {
        (0.0, 0.0)
    } else {
        (
            f0_values.iter().copied().fold(f32::INFINITY, f32::min),
            f0_values.iter().copied().fold(0.0, f32::max),
        )
    };
    let (spectral_centroid, formants, mfcc) = spectral_features(samples, sample_rate);
    let jitter = normalized_stddev(&f0_values);
    let shimmer = normalized_stddev(&rms_values);
    let harmonic_to_noise_ratio = hnr_db(&frames, sample_rate);
    let speech_rate = estimate_speech_rate(samples, sample_rate);
    let quality_score = quality_score(samples, fundamental_frequency);

    VoiceFeatures {
        fundamental_frequency,
        frequency_range,
        formant_frequencies: formants,
        speech_rate,
        mfcc,
        spectral_centroid,
        jitter,
        shimmer,
        harmonic_to_noise_ratio,
        quality_score,
    }
}

fn voiced_frames(samples: &[f32], sample_rate: u32) -> Vec<&[f32]> {
    let frame_len = (sample_rate as usize / 40).max(256);
    let hop = (frame_len / 2).max(1);
    samples
        .windows(frame_len)
        .step_by(hop)
        .filter(|frame| rms(frame) > 0.01)
        .collect()
}

fn estimate_pitch(frame: &[f32], sample_rate: u32) -> Option<(f32, f32)> {
    let min_lag = (sample_rate / 400).max(1) as usize;
    let max_lag = (sample_rate / 50).max(1) as usize;
    let energy = frame.iter().map(|v| v * v).sum::<f32>();
    if energy <= f32::EPSILON {
        return None;
    }
    let mut best_lag = 0;
    let mut best_corr = 0.0;
    for lag in min_lag..max_lag.min(frame.len().saturating_sub(1)) {
        let corr = frame
            .iter()
            .zip(frame.iter().skip(lag))
            .map(|(a, b)| a * b)
            .sum::<f32>()
            / energy;
        if corr > best_corr {
            best_corr = corr;
            best_lag = lag;
        }
    }
    (best_lag > 0 && best_corr > 0.25).then_some((sample_rate as f32 / best_lag as f32, best_corr))
}

fn spectral_features(samples: &[f32], sample_rate: u32) -> (f32, Vec<f32>, Vec<f32>) {
    let window_len = samples.len().min(1024);
    if window_len < 32 {
        return (0.0, Vec::new(), vec![0.0; 13]);
    }
    let window = &samples[..window_len];
    let bins = window_len / 2;
    let mut magnitudes = Vec::with_capacity(bins);
    for bin in 1..bins {
        let mut re = 0.0;
        let mut im = 0.0;
        for (n, sample) in window.iter().enumerate() {
            let angle = 2.0 * PI * bin as f32 * n as f32 / window_len as f32;
            re += sample * angle.cos();
            im -= sample * angle.sin();
        }
        let freq = bin as f32 * sample_rate as f32 / window_len as f32;
        magnitudes.push((freq, (re * re + im * im).sqrt()));
    }

    let total_mag = magnitudes.iter().map(|(_, mag)| mag).sum::<f32>();
    let centroid = if total_mag <= f32::EPSILON {
        0.0
    } else {
        magnitudes.iter().map(|(freq, mag)| freq * mag).sum::<f32>() / total_mag
    };

    let mut peaks = magnitudes
        .iter()
        .filter(|(freq, _)| (300.0..=3500.0).contains(freq))
        .copied()
        .collect::<Vec<_>>();
    peaks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let mut formants = peaks
        .into_iter()
        .map(|(freq, _)| freq)
        .take(3)
        .collect::<Vec<_>>();
    formants.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let band_count = 13;
    let max_freq = sample_rate as f32 / 2.0;
    let mut mfcc = Vec::with_capacity(band_count);
    for band in 0..band_count {
        let start = band as f32 * max_freq / band_count as f32;
        let end = (band + 1) as f32 * max_freq / band_count as f32;
        let energy = magnitudes
            .iter()
            .filter(|(freq, _)| *freq >= start && *freq < end)
            .map(|(_, mag)| mag * mag)
            .sum::<f32>();
        mfcc.push((energy + 1e-6).ln());
    }

    (centroid, formants, mfcc)
}

fn hnr_db(frames: &[&[f32]], sample_rate: u32) -> f32 {
    let correlations = frames
        .iter()
        .filter_map(|frame| {
            estimate_pitch(frame, sample_rate).map(|(_, corr)| corr.clamp(0.0, 0.99))
        })
        .collect::<Vec<_>>();
    let harmonic = mean_or(&correlations, 0.0);
    if harmonic <= 0.0 {
        return 0.0;
    }
    10.0 * (harmonic / (1.0 - harmonic).max(1e-3)).log10()
}

fn estimate_speech_rate(samples: &[f32], sample_rate: u32) -> f32 {
    let frame_len = (sample_rate as usize / 20).max(1);
    let threshold = rms(samples) * 0.6;
    let mut segments = 0;
    let mut in_voice = false;
    for frame in samples.chunks(frame_len) {
        let active = rms(frame) > threshold.max(0.01);
        if active && !in_voice {
            segments += 1;
        }
        in_voice = active;
    }
    let seconds = samples.len() as f32 / sample_rate as f32;
    if seconds > 0.0 {
        segments as f32 / seconds
    } else {
        0.0
    }
}

fn quality_score(samples: &[f32], fundamental_frequency: f32) -> f32 {
    let level = (rms(samples) * 20.0).clamp(0.0, 1.0);
    let voiced = if fundamental_frequency > 0.0 {
        1.0
    } else {
        0.35
    };
    (level * voiced).clamp(0.0, 1.0)
}

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|v| v * v).sum::<f32>() / samples.len() as f32).sqrt()
}

fn mean_or(values: &[f32], default: f32) -> f32 {
    if values.is_empty() {
        default
    } else {
        values.iter().sum::<f32>() / values.len() as f32
    }
}

fn normalized_stddev(values: &[f32]) -> f32 {
    if values.len() < 2 {
        return 0.0;
    }
    let mean = mean_or(values, 0.0);
    if mean.abs() <= f32::EPSILON {
        return 0.0;
    }
    let variance = values
        .iter()
        .map(|value| {
            let diff = value - mean;
            diff * diff
        })
        .sum::<f32>()
        / values.len() as f32;
    variance.sqrt() / mean.abs()
}

fn duration_ms(sample_count: usize, sample_rate: u32) -> u32 {
    ((sample_count as u64 * 1000) / u64::from(sample_rate)).min(u64::from(u32::MAX)) as u32
}

fn voice_id_from_embedding(embedding: &[f32]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for value in embedding {
        for byte in value.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    format!("speaker:{hash:016x}")
}

fn audio_timestamp(clip: &GraphVoiceClip) -> Option<DateTime<Utc>> {
    clip.clip
        .captured_at
        .as_deref()
        .and_then(parse_observed_at)
        .or_else(|| clip.occurred_at.as_deref().and_then(parse_observed_at))
}

fn default_voice_embedding_model_path() -> Option<String> {
    PathBuf::from(DEFAULT_VOICE_EMBEDDING_MODEL_PATH)
        .exists()
        .then(|| DEFAULT_VOICE_EMBEDDING_MODEL_PATH.to_string())
}

fn decode_audio_clip_samples(
    clip: &psyche::AudioClip,
    target_sample_rate: u32,
) -> anyhow::Result<Vec<f32>> {
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
        bail!("unsupported audio clip MIME type {}", clip.mime);
    };

    if sample_rate != target_sample_rate {
        bail!("audio clip sample rate {sample_rate} does not match expected {target_sample_rate}");
    }

    Ok(downmix_to_mono(samples, channels))
}

fn decode_wav_samples(bytes: &[u8]) -> anyhow::Result<(u32, u16, Vec<f32>)> {
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
        _ => bail!("unsupported WAV bit depth {}", spec.bits_per_sample),
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

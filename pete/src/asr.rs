use std::collections::VecDeque;
use std::env;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use hound::{SampleFormat, WavSpec, WavWriter};
use serde::Serialize;
use tokio::sync::mpsc;
use tokio::time::{MissedTickBehavior, interval};
use tracing::{error, info};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub const DEFAULT_MODEL_PATH: &str = "models/whisper/ggml-base.en.bin";

#[derive(Clone)]
pub struct AsrService {
    context: Arc<Mutex<WhisperContext>>,
    sample_rate: u32,
    stability_hits: usize,
    hop: Duration,
    min_duration: Duration,
    finality_lag: Duration,
    silence_threshold: f32,
    silence_duration: Duration,
}

#[derive(Clone, Debug, Serialize)]
pub struct WordTiming {
    pub text: String,
    pub start_ms: u32,
    pub end_ms: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct SegmentMessage {
    pub text: String,
    pub start_ms: u32,
    pub end_ms: u32,
    pub words: Vec<WordTiming>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AsrTranscript {
    pub text: String,
    pub start_ms: u32,
    pub end_ms: u32,
    pub audio_base64: String,
    pub segments: Vec<SegmentMessage>,
}

#[derive(Debug, Clone)]
struct SegmentInternal {
    text: String,
    start_s: f32,
    end_s: f32,
    words: Vec<WordTiming>,
}

#[derive(Debug, Clone)]
struct TrackedSegment {
    text: String,
    start_s: f32,
    end_s: f32,
    stability: usize,
}

impl AsrService {
    pub fn from_env() -> Result<Option<Self>> {
        let model_path = match env::var("WHISPER_MODEL") {
            Ok(path) => PathBuf::from(path),
            Err(_) => {
                let default = PathBuf::from(DEFAULT_MODEL_PATH);
                if default.exists() {
                    default
                } else {
                    info!(
                        default_model_path = DEFAULT_MODEL_PATH,
                        "Whisper model not found; run `cargo run -p xtask -- fetch-asr-model` or set WHISPER_MODEL"
                    );
                    return Ok(None);
                }
            }
        };
        let sample_rate = parse_env("ASR_SAMPLE_RATE", 16_000)?;
        let stability_hits = parse_env("ASR_STABILITY_HITS", 2usize)?.max(1);
        let hop_ms = parse_env("ASR_HOP_MS", 750u64)?.max(100);
        let min_duration_ms = parse_env("ASR_MIN_DURATION_MS", 2_000u64)?.max(100);
        let finality_lag_ms = parse_env("ASR_FINALITY_LAG_MS", 900u64)?.clamp(100, 10_000);
        let silence_threshold = parse_env("ASR_SILENCE_THRESHOLD", 0.015f32)?.clamp(0.0, 1.0);
        let silence_duration_ms =
            parse_env("ASR_SILENCE_DURATION_MS", 1_200u64)?.clamp(100, 10_000);

        info!(model_path = %model_path.display(), "loading whisper model");
        let context = WhisperContext::new_with_params(
            model_path
                .to_str()
                .ok_or_else(|| anyhow!("WHISPER_MODEL path is not valid UTF-8"))?,
            WhisperContextParameters::default(),
        )
        .with_context(|| format!("failed to load whisper model from {}", model_path.display()))?;

        Ok(Some(Self {
            context: Arc::new(Mutex::new(context)),
            sample_rate,
            stability_hits,
            hop: Duration::from_millis(hop_ms),
            min_duration: Duration::from_millis(min_duration_ms),
            finality_lag: Duration::from_millis(finality_lag_ms),
            silence_threshold,
            silence_duration: Duration::from_millis(silence_duration_ms),
        }))
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn spawn_connection(
        self: Arc<Self>,
    ) -> (mpsc::Sender<Vec<u8>>, mpsc::Receiver<AsrTranscript>) {
        let (pcm_tx, pcm_rx) = mpsc::channel(64);
        let (transcript_tx, transcript_rx) = mpsc::channel(64);
        tokio::spawn(async move {
            if let Err(err) = run_connection(self, pcm_rx, transcript_tx).await {
                error!(error = %err, "ASR connection failed");
            }
        });
        (pcm_tx, transcript_rx)
    }

    async fn transcribe(&self, audio: Vec<f32>) -> Result<Vec<SegmentInternal>> {
        let ctx = self.context.clone();
        Ok(tokio::task::spawn_blocking(move || {
            let guard = ctx
                .lock()
                .map_err(|_| anyhow!("failed to lock whisper context"))?;
            let mut state = guard
                .create_state()
                .context("failed to create whisper state")?;

            let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);
            params.set_token_timestamps(true);
            params.set_no_context(true);
            params.set_single_segment(false);
            params.set_n_threads(std::cmp::max(1, num_cpus::get() as i32 - 1));

            state
                .full(params, &audio)
                .context("whisper full() failed")?;

            let mut segments = Vec::new();
            let count = state.full_n_segments();
            for s in 0..count {
                let segment = state
                    .get_segment(s)
                    .ok_or_else(|| anyhow!("missing whisper segment {s}"))?;
                let text = segment.to_str_lossy()?.to_string();
                let start_s = segment.start_timestamp() as f32 / 100.0;
                let end_s = segment.end_timestamp() as f32 / 100.0;
                let mut words = Vec::new();
                let token_count = segment.n_tokens();
                let mut current = String::new();
                let mut word_start = None;
                for t in 0..token_count {
                    let token = segment
                        .get_token(t)
                        .ok_or_else(|| anyhow!("missing whisper token {t}"))?;
                    let token_data = token.token_data();
                    let piece = token.to_str_lossy()?.to_string();
                    let clean = piece.replace('▁', " ");
                    if piece.contains('▁') {
                        if let Some(start) = word_start.take() {
                            let word = current.trim().to_string();
                            if !word.is_empty() {
                                let end = token_data.t0 as f32 / 100.0;
                                words.push(WordTiming {
                                    text: word,
                                    start_ms: (start * 1000.0) as u32,
                                    end_ms: (end * 1000.0) as u32,
                                });
                            }
                            current.clear();
                        }
                        word_start = Some(token_data.t0 as f32 / 100.0);
                    } else if word_start.is_none() {
                        word_start = Some(token_data.t0 as f32 / 100.0);
                    }
                    current.push_str(&clean);
                    let end = token_data.t1 as f32 / 100.0;
                    if end > start_s
                        && current
                            .trim()
                            .ends_with(|c: char| c == ' ' || c == ',' || c == '.')
                    {
                        if let Some(start) = word_start.take() {
                            let word = current.trim().to_string();
                            if !word.is_empty() {
                                words.push(WordTiming {
                                    text: word,
                                    start_ms: (start * 1000.0) as u32,
                                    end_ms: (end * 1000.0) as u32,
                                });
                            }
                            current.clear();
                        }
                    }
                }
                if let Some(start) = word_start {
                    let word = current.trim().to_string();
                    if !word.is_empty() {
                        words.push(WordTiming {
                            text: word,
                            start_ms: (start * 1000.0) as u32,
                            end_ms: (end_s * 1000.0) as u32,
                        });
                    }
                }

                segments.push(SegmentInternal {
                    text: text.trim().to_string(),
                    start_s,
                    end_s,
                    words,
                });
            }

            Ok::<_, anyhow::Error>(segments)
        })
        .await??)
    }
}

fn parse_env<T>(key: &str, default: T) -> Result<T>
where
    T: std::str::FromStr + ToString,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    env::var(key)
        .unwrap_or_else(|_| default_to_string(&default))
        .parse()
        .with_context(|| format!("invalid {key}"))
}

fn default_to_string<T>(value: &T) -> String
where
    T: ToString,
{
    value.to_string()
}

async fn run_connection(
    service: Arc<AsrService>,
    mut pcm_rx: mpsc::Receiver<Vec<u8>>,
    out_tx: mpsc::Sender<AsrTranscript>,
) -> Result<()> {
    let mut buffer = VecDeque::<f32>::new();
    let mut tracked: Vec<TrackedSegment> = Vec::new();
    let mut total_consumed_samples = 0usize;
    let sample_rate = service.sample_rate as f32;
    let mut silence_tracker = SilenceTracker::new(
        service.sample_rate,
        service.silence_threshold,
        service.silence_duration,
    );
    let min_samples = (service.min_duration.as_secs_f32() * sample_rate) as usize;
    let mut pcm_open = true;
    let mut ticker = interval(service.hop);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    while pcm_open || !buffer.is_empty() {
        tokio::select! {
            chunk = pcm_rx.recv(), if pcm_open => {
                match chunk {
                    Some(bytes) => {
                        let (samples, rms) = extend_buffer(&mut buffer, &bytes);
                        silence_tracker.ingest(rms, samples);
                    }
                    None => pcm_open = false,
                }
            }
            _ = ticker.tick() => {
                if buffer.len() < min_samples && pcm_open {
                    continue;
                }
                if buffer.is_empty() {
                    if !pcm_open {
                        break;
                    }
                    continue;
                }

                let audio = buffer.iter().copied().collect::<Vec<_>>();
                let (trimmed_audio, leading_trim) = trim_silence(
                    &audio,
                    service.sample_rate,
                    service.silence_threshold,
                    service.silence_duration,
                );

                if trimmed_audio.is_empty() {
                    if silence_tracker.has_boundary() && !buffer.is_empty() {
                        let dropped = buffer.len();
                        buffer.clear();
                        total_consumed_samples += dropped;
                        silence_tracker.reset();
                        tracked.clear();
                    }
                    continue;
                }

                match service.transcribe(trimmed_audio).await {
                    Ok(segments) => {
                        let silence_boundary = silence_tracker.has_boundary();
                        if segments.is_empty() {
                            if silence_boundary && !buffer.is_empty() {
                                let dropped = buffer.len();
                                buffer.clear();
                                total_consumed_samples += dropped;
                                silence_tracker.reset();
                                tracked.clear();
                            }
                            continue;
                        }

                        let buffer_duration =
                            buffer.len().saturating_sub(leading_trim) as f32 / sample_rate;
                        let (finalized, mut new_tracked) = reconcile_segments(
                            &segments,
                            &tracked,
                            service.stability_hits,
                            buffer_duration,
                            service.finality_lag.as_secs_f32(),
                            silence_boundary,
                        );

                        if !finalized.is_empty() {
                            let final_end = finalized
                                .iter()
                                .map(|seg| seg.end_s)
                                .fold(0.0f32, f32::max);
                            let final_samples = (final_end * sample_rate).round() as usize;

                            if final_samples > 0
                                && final_samples <= buffer.len().saturating_sub(leading_trim)
                            {
                                let global_offset =
                                    (total_consumed_samples + leading_trim) as f32 / sample_rate;
                                let wav_audio = buffer
                                    .iter()
                                    .skip(leading_trim)
                                    .take(final_samples)
                                    .copied()
                                    .collect::<Vec<_>>();
                                let wav_bytes = encode_wav(&wav_audio, service.sample_rate)
                                    .context("failed to encode wav")?;

                                if silence_boundary {
                                    let dropped = buffer.len();
                                    buffer.clear();
                                    total_consumed_samples += dropped;
                                    silence_tracker.reset();
                                    new_tracked.clear();
                                } else {
                                    let drain_amount = leading_trim + final_samples;
                                    buffer.drain(..drain_amount);
                                    total_consumed_samples += drain_amount;
                                    for seg in &mut new_tracked {
                                        seg.start_s = (seg.start_s - final_end).max(0.0);
                                        seg.end_s = (seg.end_s - final_end).max(0.0);
                                    }
                                }

                                let text = finalized
                                    .iter()
                                    .map(|s| s.text.trim())
                                    .filter(|s| !s.is_empty())
                                    .collect::<Vec<_>>()
                                    .join(" ");
                                if let (Some(first), Some(last)) =
                                    (finalized.first(), finalized.last())
                                {
                                    let transcript = AsrTranscript {
                                        text,
                                        start_ms: ((global_offset + first.start_s) * 1000.0) as u32,
                                        end_ms: ((global_offset + last.end_s) * 1000.0) as u32,
                                        audio_base64: BASE64_STANDARD.encode(&wav_bytes),
                                        segments: finalized
                                            .iter()
                                            .map(|seg| segment_to_message(seg, global_offset))
                                            .collect(),
                                    };
                                    let _ = out_tx.send(transcript).await;
                                }
                            }
                        } else if silence_boundary && !buffer.is_empty() {
                            let dropped = buffer.len();
                            buffer.clear();
                            total_consumed_samples += dropped;
                            silence_tracker.reset();
                            new_tracked.clear();
                        }

                        tracked = new_tracked;
                    }
                    Err(err) => {
                        error!(error = %err, "transcription error");
                        tokio::time::sleep(Duration::from_millis(250)).await;
                    }
                }
            }
        }
    }

    Ok(())
}

struct SilenceTracker {
    threshold: f32,
    required_samples: usize,
    tail_silence_samples: usize,
}

impl SilenceTracker {
    fn new(sample_rate: u32, threshold: f32, duration: Duration) -> Self {
        let mut required_samples = (sample_rate as f32 * duration.as_secs_f32()).round() as usize;
        if required_samples == 0 {
            required_samples = (sample_rate as f32 * 0.2).round() as usize;
        }
        Self {
            threshold,
            required_samples,
            tail_silence_samples: 0,
        }
    }

    fn ingest(&mut self, rms: f32, samples: usize) {
        if samples == 0 {
            return;
        }
        if rms <= self.threshold {
            self.tail_silence_samples = self.tail_silence_samples.saturating_add(samples);
        } else {
            self.tail_silence_samples = 0;
        }
    }

    fn has_boundary(&self) -> bool {
        self.required_samples > 0 && self.tail_silence_samples >= self.required_samples
    }

    fn reset(&mut self) {
        self.tail_silence_samples = 0;
    }
}

fn extend_buffer(buffer: &mut VecDeque<f32>, bytes: &[u8]) -> (usize, f32) {
    let mut sum_sq = 0.0f64;
    let mut count = 0usize;
    for chunk in bytes.chunks_exact(2) {
        let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
        let normalized = sample as f32 / i16::MAX as f32;
        let value = f64::from(normalized);
        sum_sq += value * value;
        buffer.push_back(normalized);
        count += 1;
    }
    let rms = if count > 0 {
        (sum_sq / count as f64).sqrt() as f32
    } else {
        0.0
    };
    (count, rms)
}

fn trim_silence(
    samples: &[f32],
    sample_rate: u32,
    threshold: f32,
    silence_duration: Duration,
) -> (Vec<f32>, usize) {
    let len = samples.len();
    if len == 0 {
        return (Vec::new(), 0);
    }

    let window_ms = silence_duration.as_millis().clamp(20, 500) as usize;
    let window = ((sample_rate as usize) * window_ms / 1000).max(1);
    let step = (window / 2).max(1);

    let mut start = 0usize;
    while start + window <= len {
        let rms = rms(&samples[start..start + window]);
        if rms > threshold {
            break;
        }
        start = (start + step).min(len);
    }

    if start >= len {
        return (Vec::new(), len);
    }

    let mut end = len;
    while end >= window && end > start {
        let from = end.saturating_sub(window);
        let rms = rms(&samples[from..end]);
        if rms > threshold {
            break;
        }
        end = end.saturating_sub(step);
    }

    if end <= start {
        return (Vec::new(), len);
    }

    (samples[start..end].to_vec(), start)
}

fn rms(samples: &[f32]) -> f32 {
    let sum_sq = samples
        .iter()
        .map(|s| {
            let v = f64::from(*s);
            v * v
        })
        .sum::<f64>();
    (sum_sq / samples.len().max(1) as f64).sqrt() as f32
}

fn segment_to_message(segment: &SegmentInternal, offset_seconds: f32) -> SegmentMessage {
    let offset_ms = (offset_seconds * 1000.0) as u32;
    SegmentMessage {
        text: segment.text.clone(),
        start_ms: ((offset_seconds + segment.start_s) * 1000.0) as u32,
        end_ms: ((offset_seconds + segment.end_s) * 1000.0) as u32,
        words: segment
            .words
            .iter()
            .map(|word| WordTiming {
                text: word.text.clone(),
                start_ms: word.start_ms + offset_ms,
                end_ms: word.end_ms + offset_ms,
            })
            .collect(),
    }
}

fn reconcile_segments(
    latest: &[SegmentInternal],
    tracked: &[TrackedSegment],
    stability_threshold: usize,
    buffer_duration: f32,
    finality_lag_s: f32,
    force_finalize: bool,
) -> (Vec<SegmentInternal>, Vec<TrackedSegment>) {
    if force_finalize {
        return (latest.to_vec(), Vec::new());
    }

    let mut prefix = Vec::new();
    let mut new_tracked = Vec::new();

    for (idx, seg) in latest.iter().enumerate() {
        let mut stability = 1;
        if let Some(prev) = tracked.get(idx) {
            if prev.text == seg.text {
                stability = prev.stability + 1;
            }
        }
        let segment_age = (buffer_duration - seg.end_s).max(0.0);
        let aged_out = finality_lag_s <= 0.0 || segment_age >= finality_lag_s;
        if idx == prefix.len() && (stability >= stability_threshold || aged_out) {
            prefix.push(seg.clone());
            continue;
        }
        new_tracked.push(TrackedSegment {
            text: seg.text.clone(),
            start_s: seg.start_s,
            end_s: seg.end_s,
            stability,
        });
    }

    (prefix, new_tracked)
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

#[cfg(test)]
mod tests {
    use super::{SegmentInternal, SilenceTracker, TrackedSegment, WordTiming, reconcile_segments};
    use std::time::Duration;

    fn make_segment(text: &str, start: f32, end: f32) -> SegmentInternal {
        SegmentInternal {
            text: text.to_string(),
            start_s: start,
            end_s: end,
            words: Vec::<WordTiming>::new(),
        }
    }

    #[test]
    fn reconcile_segments_promotes_stable_prefix() {
        let latest = vec![
            make_segment("hello", 0.0, 1.0),
            make_segment("world", 1.0, 2.0),
        ];
        let tracked = vec![
            TrackedSegment {
                text: "hello".to_string(),
                start_s: 0.0,
                end_s: 1.0,
                stability: 2,
            },
            TrackedSegment {
                text: "world".to_string(),
                start_s: 1.0,
                end_s: 2.0,
                stability: 1,
            },
        ];

        let (finalized, remaining) = reconcile_segments(&latest, &tracked, 3, 2.5, 0.9, false);

        assert_eq!(finalized.len(), 1);
        assert_eq!(finalized[0].text, "hello");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].text, "world");
        assert_eq!(remaining[0].stability, 2);
    }

    #[test]
    fn reconcile_segments_emits_when_audio_has_aged_out() {
        let latest = vec![make_segment("hello there", 0.0, 1.2)];
        let tracked = vec![TrackedSegment {
            text: "hello".to_string(),
            start_s: 0.0,
            end_s: 1.2,
            stability: 1,
        }];

        let (finalized, remaining) = reconcile_segments(&latest, &tracked, 4, 5.0, 0.5, false);

        assert_eq!(finalized.len(), 1);
        assert!(remaining.is_empty());
    }

    #[test]
    fn reconcile_segments_can_force_finalization() {
        let latest = vec![make_segment("forced", 0.0, 0.8)];
        let tracked = vec![TrackedSegment {
            text: "forced".to_string(),
            start_s: 0.0,
            end_s: 0.8,
            stability: 1,
        }];

        let (finalized, remaining) = reconcile_segments(&latest, &tracked, 10, 2.0, 5.0, true);

        assert_eq!(finalized.len(), 1);
        assert!(remaining.is_empty());
    }

    #[test]
    fn silence_tracker_detects_required_duration() {
        let mut tracker = SilenceTracker::new(16_000, 0.01, Duration::from_millis(500));
        assert!(!tracker.has_boundary());

        tracker.ingest(0.005, 8_000);
        assert!(tracker.has_boundary());

        tracker.reset();
        tracker.ingest(0.02, 8_000);
        assert!(!tracker.has_boundary());
    }
}

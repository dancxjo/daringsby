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
use tracing::{debug, error, info};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub const DEFAULT_MODEL_PATH: &str = "models/whisper/ggml-base.en.bin";
const DEFAULT_PCM_QUEUE_CAPACITY: usize = 4;

#[derive(Clone)]
pub struct AsrService {
    context: Arc<Mutex<WhisperContext>>,
    sample_rate: u32,
    hop: Duration,
    min_duration: Duration,
    max_buffer_duration: Duration,
    silence_threshold: f32,
    silence_duration: Duration,
    pcm_queue_capacity: usize,
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
        let hop_ms = parse_env("ASR_HOP_MS", 250u64)?.max(100);
        let min_duration_ms = parse_env("ASR_MIN_DURATION_MS", 2_000u64)?.max(100);
        let max_buffer_ms =
            parse_env("ASR_MAX_BUFFER_MS", 8_000u64)?.clamp(min_duration_ms, 60_000);
        let silence_threshold = parse_env("ASR_SILENCE_THRESHOLD", 0.015f32)?.clamp(0.0, 1.0);
        let silence_duration_ms =
            parse_env("ASR_SILENCE_DURATION_MS", 1_200u64)?.clamp(100, 10_000);
        let pcm_queue_capacity =
            parse_env("ASR_PCM_QUEUE_CAPACITY", DEFAULT_PCM_QUEUE_CAPACITY)?.clamp(1usize, 64usize);

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
            hop: Duration::from_millis(hop_ms),
            min_duration: Duration::from_millis(min_duration_ms),
            max_buffer_duration: Duration::from_millis(max_buffer_ms),
            silence_threshold,
            silence_duration: Duration::from_millis(silence_duration_ms),
            pcm_queue_capacity,
        }))
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn spawn_connection(
        self: Arc<Self>,
    ) -> (mpsc::Sender<Vec<u8>>, mpsc::Receiver<AsrTranscript>) {
        let (pcm_tx, pcm_rx) = mpsc::channel(self.pcm_queue_capacity);
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
    let mut total_consumed_samples = 0usize;
    let sample_rate = service.sample_rate as f32;
    let mut silence_tracker = SilenceTracker::new(
        service.sample_rate,
        service.silence_threshold,
        service.silence_duration,
    );
    let min_samples = (service.min_duration.as_secs_f32() * sample_rate) as usize;
    let max_samples = (service.max_buffer_duration.as_secs_f32() * sample_rate) as usize;
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
                if buffer.is_empty() {
                    if !pcm_open {
                        break;
                    }
                    continue;
                }

                let has_pause = silence_tracker.has_boundary();
                let hit_max_buffer = buffer.len() >= max_samples;
                let should_transcribe =
                    !pcm_open || (buffer.len() >= min_samples && (has_pause || hit_max_buffer));
                if !should_transcribe {
                    continue;
                }

                let audio = buffer.iter().copied().collect::<Vec<_>>();
                let utterance_start_samples = total_consumed_samples;
                let submitted_samples = buffer.len();
                let (trimmed_audio, leading_trim) = trim_silence(
                    &audio,
                    service.sample_rate,
                    service.silence_threshold,
                    service.silence_duration,
                );

                buffer.clear();
                total_consumed_samples += submitted_samples;
                let dropped_pending_samples = drain_pending_pcm(&mut pcm_rx);
                total_consumed_samples += dropped_pending_samples;
                silence_tracker.reset();

                if trimmed_audio.is_empty() {
                    debug!(
                        submitted_samples,
                        dropped_pending_samples,
                        has_pause,
                        hit_max_buffer,
                        "dropping silent ASR utterance"
                    );
                    continue;
                }

                let global_offset =
                    (utterance_start_samples + leading_trim) as f32 / sample_rate;
                let wav_bytes = encode_wav(&trimmed_audio, service.sample_rate)
                    .context("failed to encode wav")?;

                match service.transcribe(trimmed_audio).await {
                    Ok(segments) => {
                        emit_transcript(segments, global_offset, wav_bytes, &out_tx).await;
                    }
                    Err(err) => {
                        error!(error = %err, "transcription error");
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

fn drain_pending_pcm(pcm_rx: &mut mpsc::Receiver<Vec<u8>>) -> usize {
    let mut samples = 0usize;
    while let Ok(bytes) = pcm_rx.try_recv() {
        samples += bytes.len() / 2;
    }
    samples
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

async fn emit_transcript(
    segments: Vec<SegmentInternal>,
    global_offset: f32,
    wav_bytes: Vec<u8>,
    out_tx: &mpsc::Sender<AsrTranscript>,
) {
    let segments = segments
        .into_iter()
        .filter(|segment| !segment.text.trim().is_empty())
        .collect::<Vec<_>>();
    let Some(first) = segments.first() else {
        return;
    };
    let Some(last) = segments.last() else {
        return;
    };

    let text = segments
        .iter()
        .map(|segment| segment.text.trim())
        .collect::<Vec<_>>()
        .join(" ");
    if text.is_empty() {
        return;
    }

    let transcript = AsrTranscript {
        text,
        start_ms: ((global_offset + first.start_s) * 1000.0) as u32,
        end_ms: ((global_offset + last.end_s) * 1000.0) as u32,
        audio_base64: BASE64_STANDARD.encode(&wav_bytes),
        segments: segments
            .iter()
            .map(|seg| segment_to_message(seg, global_offset))
            .collect(),
    };
    let _ = out_tx.send(transcript).await;
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
    use super::{
        SegmentInternal, SilenceTracker, WordTiming, drain_pending_pcm, emit_transcript,
        trim_silence,
    };
    use std::time::Duration;
    use tokio::sync::mpsc;

    fn make_segment(text: &str, start: f32, end: f32) -> SegmentInternal {
        SegmentInternal {
            text: text.to_string(),
            start_s: start,
            end_s: end,
            words: Vec::<WordTiming>::new(),
        }
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

    #[test]
    fn trim_silence_keeps_middle_audio() {
        let samples = [0.0, 0.0, 0.2, 0.2, 0.0, 0.0];

        let (trimmed, leading_trim) = trim_silence(&samples, 50, 0.01, Duration::from_millis(20));

        assert_eq!(trimmed, vec![0.2, 0.2]);
        assert_eq!(leading_trim, 2);
    }

    #[tokio::test]
    async fn emit_transcript_skips_empty_segments() {
        let (tx, mut rx) = mpsc::channel(1);

        emit_transcript(vec![make_segment(" ", 0.0, 1.0)], 0.0, Vec::new(), &tx).await;

        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn emit_transcript_sends_joined_text_once() {
        let (tx, mut rx) = mpsc::channel(1);

        emit_transcript(
            vec![
                make_segment("hello", 0.0, 0.5),
                make_segment("there", 0.5, 1.0),
            ],
            2.0,
            Vec::new(),
            &tx,
        )
        .await;

        let transcript = rx.try_recv().expect("transcript should be sent");
        assert_eq!(transcript.text, "hello there");
        assert_eq!(transcript.start_ms, 2000);
        assert_eq!(transcript.end_ms, 3000);
    }

    #[tokio::test]
    async fn drain_pending_pcm_discards_queued_audio() {
        let (tx, mut rx) = mpsc::channel(4);
        tx.send(vec![0, 0, 1, 0]).await.unwrap();
        tx.send(vec![2, 0, 3, 0, 4, 0]).await.unwrap();

        let drained = drain_pending_pcm(&mut rx);

        assert_eq!(drained, 5);
        assert!(rx.try_recv().is_err());
    }
}

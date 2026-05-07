use std::collections::VecDeque;
use std::env;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use chrono::{DateTime, Utc};
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use serde::Serialize;
#[cfg(feature = "voice")]
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::mpsc;
use tokio::time::{MissedTickBehavior, interval};
use tracing::{error, info, trace, warn};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use psyche::{AudioClip, Sensation, Topic, TopicBus};
#[cfg(feature = "voice")]
use psyche::{QdrantClient, VoiceInfo, audio_clip_id};

pub const DEFAULT_MODEL_PATH: &str = "models/whisper/ggml-small.en.bin";
pub const LEGACY_FAST_MODEL_PATH: &str = "models/whisper/ggml-base.en.bin";
pub const HIGH_QUALITY_MULTILINGUAL_MODEL_PATH: &str = "models/whisper/ggml-large-v3.bin";
#[cfg(feature = "voice")]
pub const DEFAULT_VOICE_EMBEDDING_MODEL_PATH: &str =
    "models/voice/speaker_embedding_extractor.onnx";
const DEFAULT_PCM_QUEUE_CAPACITY: usize = 4;

#[derive(Clone)]
pub struct AsrService {
    context: Option<Arc<Mutex<WhisperContext>>>,
    #[cfg(feature = "voice")]
    voice_embeddings: Option<Arc<VoiceEmbeddingService>>,
    sample_rate: u32,
    hop: Duration,
    min_duration: Duration,
    max_buffer_duration: Duration,
    silence_threshold: f32,
    silence_duration: Duration,
    pcm_queue_capacity: usize,
    topic_bus: Option<TopicBus>,
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
pub struct ClipTranscription {
    pub text: String,
    pub segments: Vec<SegmentMessage>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SourceClipSpan {
    pub index: usize,
    pub start_ms: u32,
    pub end_ms: u32,
    pub sample_count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct MultiClipTranscription {
    pub text: String,
    pub segments: Vec<SegmentMessage>,
    pub source_spans: Vec<SourceClipSpan>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AsrTranscript {
    pub text: String,
    pub occurred_at: DateTime<Utc>,
    pub start_ms: u32,
    pub end_ms: u32,
    pub audio_base64: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub segments: Vec<SegmentMessage>,
}

impl AsrTranscript {
    pub fn audio_clip(&self) -> AudioClip {
        AudioClip {
            mime: "audio/wav".to_string(),
            base64: self.audio_base64.clone(),
            sample_rate: self.sample_rate,
            channels: self.channels,
            transcript: Some(self.text.clone()),
            captured_at: Some(self.occurred_at.to_rfc3339()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AudioChunk {
    pub bytes: Vec<u8>,
    pub captured_at: DateTime<Utc>,
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
        let model_path = env::var("WHISPER_MODEL")
            .map(PathBuf::from)
            .ok()
            .or_else(|| {
                let default = PathBuf::from(DEFAULT_MODEL_PATH);
                default.exists().then_some(default)
            })
            .or_else(|| {
                let legacy = PathBuf::from(LEGACY_FAST_MODEL_PATH);
                legacy.exists().then_some(legacy)
            })
            .or_else(|| {
                let high_quality = PathBuf::from(HIGH_QUALITY_MULTILINGUAL_MODEL_PATH);
                high_quality.exists().then_some(high_quality)
            });
        Self::from_optional_whisper_model_path(model_path)
    }

    pub fn from_whisper_model_path(model_path: PathBuf) -> Result<Self> {
        Self::from_optional_whisper_model_path(Some(model_path))?
            .ok_or_else(|| anyhow!("Whisper model not configured"))
    }

    fn from_optional_whisper_model_path(model_path: Option<PathBuf>) -> Result<Option<Self>> {
        #[cfg(feature = "voice")]
        let voice_model_requested = voice_embedding_model_path().is_some();
        #[cfg(not(feature = "voice"))]
        let voice_model_requested = false;

        if model_path.is_none() && !voice_model_requested {
            info!(
                default_model_path = DEFAULT_MODEL_PATH,
                high_quality_model_path = HIGH_QUALITY_MULTILINGUAL_MODEL_PATH,
                "Whisper model not found and voice embeddings are not configured; run `just fetch` to enable audio analysis"
            );
            return Ok(None);
        }

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
        let whisper_use_gpu = parse_env("ASR_USE_GPU", whisper_gpu_enabled_by_default())?;
        let whisper_gpu_device = parse_env("ASR_GPU_DEVICE", 0i32)?.max(0);

        let context = if let Some(model_path) = model_path {
            info!(
                model_path = %model_path.display(),
                use_gpu = whisper_use_gpu,
                gpu_device = whisper_gpu_device,
                "loading whisper model"
            );
            let mut context_params = WhisperContextParameters::default();
            context_params
                .use_gpu(whisper_use_gpu)
                .gpu_device(whisper_gpu_device);
            Some(Arc::new(Mutex::new(
                WhisperContext::new_with_params(
                    model_path
                        .to_str()
                        .ok_or_else(|| anyhow!("WHISPER_MODEL path is not valid UTF-8"))?,
                    context_params,
                )
                .with_context(|| {
                    format!("failed to load whisper model from {}", model_path.display())
                })?,
            )))
        } else {
            info!("Whisper model not configured; audio analysis will skip transcription");
            None
        };

        Ok(Some(Self {
            context,
            #[cfg(feature = "voice")]
            voice_embeddings: None,
            sample_rate,
            hop: Duration::from_millis(hop_ms),
            min_duration: Duration::from_millis(min_duration_ms),
            max_buffer_duration: Duration::from_millis(max_buffer_ms),
            silence_threshold,
            silence_duration: Duration::from_millis(silence_duration_ms),
            pcm_queue_capacity,
            topic_bus: None,
        }))
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn has_whisper_model(&self) -> bool {
        self.context.is_some()
    }

    /// Transcribe a stored audio clip and return text with segment timings.
    ///
    /// Stored clips are expected to be 16-bit little-endian PCM or WAV audio at
    /// the service sample rate. Multi-channel clips are downmixed to mono.
    pub async fn transcribe_clip(&self, clip: &AudioClip) -> Result<ClipTranscription> {
        anyhow::ensure!(self.has_whisper_model(), "Whisper model not configured");
        let audio = decode_audio_clip_samples(clip, self.sample_rate)?;
        let segments = self.transcribe(audio).await?;
        let text = join_segments(&segments);
        Ok(ClipTranscription {
            text,
            segments: segments
                .iter()
                .flat_map(|segment| segment_to_messages(segment, 0.0))
                .collect(),
        })
    }

    /// Transcribe a stored audio clip and return Whisper's joined text.
    pub async fn transcribe_clip_text(&self, clip: &AudioClip) -> Result<String> {
        Ok(self.transcribe_clip(clip).await?.text)
    }

    /// Transcribe several stored clips as one continuous audio buffer.
    ///
    /// The returned segment timings are relative to the beginning of the
    /// concatenated audio. `source_spans` records where each original clip lands
    /// inside that larger buffer so callers can link the aggregate transcript
    /// back to the source clips.
    pub async fn transcribe_clips(&self, clips: &[AudioClip]) -> Result<MultiClipTranscription> {
        anyhow::ensure!(self.has_whisper_model(), "Whisper model not configured");
        anyhow::ensure!(!clips.is_empty(), "no audio clips supplied");

        let mut audio = Vec::new();
        let mut source_spans = Vec::with_capacity(clips.len());
        for (index, clip) in clips.iter().enumerate() {
            let samples = decode_audio_clip_samples(clip, self.sample_rate)?;
            let start_sample = audio.len();
            audio.extend(samples);
            let end_sample = audio.len();
            source_spans.push(SourceClipSpan {
                index,
                start_ms: samples_to_ms(start_sample, self.sample_rate),
                end_ms: samples_to_ms(end_sample, self.sample_rate),
                sample_count: end_sample.saturating_sub(start_sample),
            });
        }

        anyhow::ensure!(!audio.is_empty(), "audio clips contained no samples");
        let segments = self.transcribe(audio).await?;
        let text = join_segments(&segments);
        Ok(MultiClipTranscription {
            text,
            segments: segments
                .iter()
                .flat_map(|segment| segment_to_messages(segment, 0.0))
                .collect(),
            source_spans,
        })
    }

    pub fn set_topic_bus(&mut self, bus: TopicBus) {
        self.topic_bus = Some(bus);
    }

    #[cfg(feature = "voice")]
    pub fn enable_voice_embeddings_from_env(
        &mut self,
        qdrant: QdrantClient,
        bus: TopicBus,
    ) -> Result<()> {
        let model_path = match env::var("VOICE_EMBEDDING_MODEL") {
            Ok(path) if !path.trim().is_empty() => path,
            _ => match voice_embedding_model_path() {
                Some(path) => path,
                None => {
                    info!(
                        default_model_path = DEFAULT_VOICE_EMBEDDING_MODEL_PATH,
                        "voice embeddings disabled; run `just fetch` or set VOICE_EMBEDDING_MODEL"
                    );
                    return Ok(());
                }
            },
        };
        self.voice_embeddings = Some(Arc::new(VoiceEmbeddingService::new(
            model_path, qdrant, bus,
        )?));
        Ok(())
    }
    pub fn spawn_connection(
        self: Arc<Self>,
    ) -> (mpsc::Sender<AudioChunk>, mpsc::Receiver<AsrTranscript>) {
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
        let Some(ctx) = self.context.clone() else {
            return Ok(Vec::new());
        };
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
            params.set_split_on_word(true);
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
                let mut word_end = None;
                for t in 0..token_count {
                    let token = segment
                        .get_token(t)
                        .ok_or_else(|| anyhow!("missing whisper token {t}"))?;
                    let token_data = token.token_data();
                    let piece = token.to_str_lossy()?.to_string();
                    push_word_token(
                        &mut words,
                        &mut current,
                        &mut word_start,
                        &mut word_end,
                        &piece,
                        centiseconds_to_ms(token_data.t0),
                        centiseconds_to_ms(token_data.t1),
                    );
                }
                finish_word(
                    &mut words,
                    &mut current,
                    &mut word_start,
                    word_end.unwrap_or_else(|| seconds_to_ms(end_s)),
                );

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

#[cfg(feature = "voice")]
fn voice_embedding_model_path() -> Option<String> {
    env::var("VOICE_EMBEDDING_MODEL")
        .ok()
        .filter(|path| !path.trim().is_empty())
        .or_else(|| {
            PathBuf::from(DEFAULT_VOICE_EMBEDDING_MODEL_PATH)
                .exists()
                .then(|| DEFAULT_VOICE_EMBEDDING_MODEL_PATH.to_string())
        })
}

#[cfg(feature = "voice")]
struct VoiceEmbeddingService {
    extractor: AsyncMutex<voxudio::SpeakerEmbeddingExtractor>,
    qdrant: QdrantClient,
    bus: TopicBus,
    model: String,
}

#[cfg(feature = "voice")]
impl VoiceEmbeddingService {
    fn new(model_path: String, qdrant: QdrantClient, bus: TopicBus) -> Result<Self> {
        let extractor = voxudio::SpeakerEmbeddingExtractor::new(&model_path)
            .with_context(|| format!("failed to load voice embedding model {model_path}"))?;
        info!(%model_path, "voice embedding model loaded");
        Ok(Self {
            extractor: AsyncMutex::new(extractor),
            qdrant,
            bus,
            model: model_path,
        })
    }

    async fn process(
        &self,
        audio_16k: &[f32],
        wav_bytes: &[u8],
        occurred_at: DateTime<Utc>,
    ) -> Result<()> {
        if audio_16k.is_empty() {
            return Ok(());
        }
        let audio_22050 = voxudio::resample::<16000, 22050, f32>(audio_16k, 1, 1)
            .context("failed to resample audio for voice embedding")?;
        if audio_22050.is_empty() {
            return Ok(());
        }
        let embeddings = self
            .extractor
            .lock()
            .await
            .extract(&audio_22050, 1)
            .await
            .context("failed to extract voice embedding")?;
        let Some(embedding) = embeddings.into_iter().next() else {
            return Ok(());
        };
        let clip = AudioClip {
            mime: "audio/wav".to_string(),
            base64: BASE64_STANDARD.encode(wav_bytes),
            sample_rate: 16_000,
            channels: 1,
            transcript: None,
            captured_at: Some(occurred_at.to_rfc3339()),
        };
        let clip_id = audio_clip_id(&clip);
        let embedding = embedding.to_vec();
        let vector_id = self
            .qdrant
            .store_voice_vector_for(Some(&clip_id), &embedding)
            .await
            .context("failed to store voice embedding")?
            .to_string();
        self.bus.publish(
            psyche::Topic::Sensation,
            Sensation::of_at(
                VoiceInfo {
                    clip,
                    clip_id,
                    embedding,
                    vector_id: Some(vector_id),
                    model: Some(self.model.clone()),
                },
                occurred_at,
            ),
        );
        Ok(())
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

fn whisper_gpu_enabled_by_default() -> bool {
    cfg!(feature = "asr-cuda")
}

fn default_to_string<T>(value: &T) -> String
where
    T: ToString,
{
    value.to_string()
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

    if sample_rate != target_sample_rate {
        return Err(anyhow!(
            "audio clip sample rate {sample_rate} does not match Whisper sample rate {target_sample_rate}"
        ));
    }

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

fn join_segments(segments: &[SegmentInternal]) -> String {
    segments
        .iter()
        .map(|segment| segment.text.trim())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

async fn run_connection(
    service: Arc<AsrService>,
    mut pcm_rx: mpsc::Receiver<AudioChunk>,
    out_tx: mpsc::Sender<AsrTranscript>,
) -> Result<()> {
    let mut buffer = VecDeque::<f32>::new();
    let mut buffer_started_at = None::<DateTime<Utc>>;
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
                    Some(chunk) => {
                        if buffer.is_empty() {
                            buffer_started_at = Some(chunk.captured_at);
                        }
                        let (samples, rms) = extend_buffer(&mut buffer, &chunk.bytes);
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
                let utterance_started_at = buffer_started_at.unwrap_or_else(Utc::now);
                let submitted_samples = buffer.len();
                let (trimmed_audio, leading_trim) = trim_silence(
                    &audio,
                    service.sample_rate,
                    service.silence_threshold,
                    service.silence_duration,
                );

                buffer.clear();
                buffer_started_at = None;
                total_consumed_samples += submitted_samples;
                silence_tracker.reset();

                if trimmed_audio.is_empty() {
                    trace!(
                        submitted_samples,
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
                let leading_trim_ms = ((leading_trim as f32 / sample_rate) * 1000.0) as i64;
                let occurred_at =
                    utterance_started_at + chrono::Duration::milliseconds(leading_trim_ms);
                let service = Arc::clone(&service);
                let out_tx = out_tx.clone();
                tokio::spawn(async move {
                    process_utterance(
                        service,
                        trimmed_audio,
                        occurred_at,
                        global_offset,
                        wav_bytes,
                        out_tx,
                    )
                    .await;
                });
            }
        }
    }

    Ok(())
}

async fn process_utterance(
    service: Arc<AsrService>,
    trimmed_audio: Vec<f32>,
    occurred_at: DateTime<Utc>,
    global_offset: f32,
    wav_bytes: Vec<u8>,
    out_tx: mpsc::Sender<AsrTranscript>,
) {
    #[cfg(feature = "voice")]
    if let Some(voice_embeddings) = &service.voice_embeddings {
        if service.sample_rate == 16_000 {
            if let Err(err) = voice_embeddings
                .process(&trimmed_audio, &wav_bytes, occurred_at)
                .await
            {
                warn!(error = %err, "voice embedding failed");
            }
        } else {
            warn!(
                sample_rate = service.sample_rate,
                "voice embeddings currently require 16 kHz ASR audio"
            );
        }
    }

    if service.context.is_none() {
        return;
    }

    match service.transcribe(trimmed_audio).await {
        Ok(segments) => {
            emit_transcript(
                segments,
                occurred_at,
                global_offset,
                wav_bytes,
                service.sample_rate,
                service.topic_bus.as_ref(),
                &out_tx,
            )
            .await;
        }
        Err(err) => {
            error!(error = %err, "transcription error");
        }
    }
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

fn centiseconds_to_ms(centiseconds: i64) -> u32 {
    centiseconds
        .max(0)
        .saturating_mul(10)
        .try_into()
        .unwrap_or(u32::MAX)
}

fn seconds_to_ms(seconds: f32) -> u32 {
    (seconds.max(0.0) * 1000.0) as u32
}

fn samples_to_ms(samples: usize, sample_rate: u32) -> u32 {
    if sample_rate == 0 {
        return 0;
    }
    let ms = (samples as u128).saturating_mul(1000) / u128::from(sample_rate);
    ms.min(u128::from(u32::MAX)) as u32
}

fn push_word_token(
    words: &mut Vec<WordTiming>,
    current: &mut String,
    word_start: &mut Option<u32>,
    word_end: &mut Option<u32>,
    piece: &str,
    start_ms: u32,
    end_ms: u32,
) {
    let starts_new_word = piece
        .chars()
        .next()
        .is_some_and(|c| c.is_whitespace() || c == '▁');
    let clean = piece.replace('▁', " ");
    if starts_new_word && !current.trim().is_empty() {
        finish_word(words, current, word_start, start_ms);
        *word_end = None;
    }
    if word_start.is_none() && !clean.trim().is_empty() {
        *word_start = Some(start_ms);
    }
    current.push_str(&clean);
    if !clean.trim().is_empty() {
        *word_end = Some(end_ms);
    }
}

fn finish_word(
    words: &mut Vec<WordTiming>,
    current: &mut String,
    word_start: &mut Option<u32>,
    end_ms: u32,
) {
    let Some(start_ms) = word_start.take() else {
        current.clear();
        return;
    };
    let text = current.trim().to_string();
    current.clear();
    if text.is_empty() {
        return;
    }
    words.push(WordTiming {
        text,
        start_ms,
        end_ms: end_ms.max(start_ms),
    });
}

fn segment_to_messages(segment: &SegmentInternal, offset_seconds: f32) -> Vec<SegmentMessage> {
    let offset_ms = (offset_seconds * 1000.0) as u32;
    let offset_word = |word: &WordTiming| WordTiming {
        text: word.text.clone(),
        start_ms: word.start_ms + offset_ms,
        end_ms: word.end_ms + offset_ms,
    };

    if !segment.words.is_empty() {
        return segment
            .words
            .iter()
            .map(|word| {
                let word = offset_word(word);
                SegmentMessage {
                    text: word.text.clone(),
                    start_ms: word.start_ms,
                    end_ms: word.end_ms,
                    words: vec![word],
                }
            })
            .collect();
    }

    vec![SegmentMessage {
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
    }]
}

async fn emit_transcript(
    segments: Vec<SegmentInternal>,
    occurred_at: DateTime<Utc>,
    global_offset: f32,
    wav_bytes: Vec<u8>,
    sample_rate: u32,
    topic_bus: Option<&TopicBus>,
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
        occurred_at: occurred_at + chrono::Duration::milliseconds((first.start_s * 1000.0) as i64),
        start_ms: ((global_offset + first.start_s) * 1000.0) as u32,
        end_ms: ((global_offset + last.end_s) * 1000.0) as u32,
        audio_base64: BASE64_STANDARD.encode(&wav_bytes),
        sample_rate,
        channels: 1,
        segments: segments
            .iter()
            .flat_map(|seg| segment_to_messages(seg, global_offset))
            .collect(),
    };
    if let Some(bus) = topic_bus {
        let mut clip = transcript.audio_clip();
        clip.captured_at = Some(occurred_at.to_rfc3339());
        bus.publish(
            Topic::Sensation,
            Sensation::of_at(clip, transcript.occurred_at),
        );
    }
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
        SegmentInternal, SilenceTracker, WordTiming, decode_audio_clip_samples, emit_transcript,
        encode_wav, finish_word, push_word_token, segment_to_messages, trim_silence,
    };
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
    use chrono::Utc;
    use futures::StreamExt;
    use psyche::{AudioClip, Sensation, Topic, TopicBus};
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
    fn word_timing_splits_on_leading_whitespace_tokens() {
        let mut words = Vec::new();
        let mut current = String::new();
        let mut word_start = None;
        let mut word_end = None;

        push_word_token(
            &mut words,
            &mut current,
            &mut word_start,
            &mut word_end,
            " hello",
            100,
            220,
        );
        push_word_token(
            &mut words,
            &mut current,
            &mut word_start,
            &mut word_end,
            " there",
            260,
            420,
        );
        push_word_token(
            &mut words,
            &mut current,
            &mut word_start,
            &mut word_end,
            ".",
            420,
            460,
        );
        finish_word(&mut words, &mut current, &mut word_start, word_end.unwrap());

        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "hello");
        assert_eq!(words[0].start_ms, 100);
        assert_eq!(words[0].end_ms, 260);
        assert_eq!(words[1].text, "there.");
        assert_eq!(words[1].start_ms, 260);
        assert_eq!(words[1].end_ms, 460);
    }

    #[test]
    fn segment_messages_prefer_word_level_segments() {
        let mut segment = make_segment("hello there", 0.0, 0.8);
        segment.words = vec![
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
        ];

        let messages = segment_to_messages(&segment, 2.0);

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].text, "hello");
        assert_eq!(messages[0].start_ms, 2000);
        assert_eq!(messages[0].end_ms, 2300);
        assert_eq!(messages[0].words.len(), 1);
        assert_eq!(messages[1].text, "there");
        assert_eq!(messages[1].start_ms, 2350);
        assert_eq!(messages[1].end_ms, 2800);
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

    #[test]
    fn decodes_pcm_clip_and_downmixes_to_mono() {
        let mut bytes = Vec::new();
        for sample in [10_000i16, -10_000, 20_000, -20_000] {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        let clip = AudioClip {
            mime: "audio/pcm;format=s16le;rate=16000".into(),
            base64: BASE64_STANDARD.encode(bytes),
            sample_rate: 16_000,
            channels: 2,
            transcript: None,
            captured_at: None,
        };

        let samples = decode_audio_clip_samples(&clip, 16_000).unwrap();

        assert_eq!(samples, vec![0.0, 0.0]);
    }

    #[test]
    fn decodes_wav_clip() {
        let wav = encode_wav(&[0.25, -0.25], 16_000).unwrap();
        let clip = AudioClip {
            mime: "audio/wav".into(),
            base64: BASE64_STANDARD.encode(wav),
            sample_rate: 16_000,
            channels: 1,
            transcript: None,
            captured_at: None,
        };

        let samples = decode_audio_clip_samples(&clip, 16_000).unwrap();

        assert_eq!(samples.len(), 2);
        assert!((samples[0] - 0.25).abs() < 0.001);
        assert!((samples[1] + 0.25).abs() < 0.001);
    }

    #[tokio::test]
    async fn emit_transcript_skips_empty_segments() {
        let (tx, mut rx) = mpsc::channel(1);

        emit_transcript(
            vec![make_segment(" ", 0.0, 1.0)],
            Utc::now(),
            0.0,
            Vec::new(),
            16_000,
            None,
            &tx,
        )
        .await;

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
            Utc::now(),
            2.0,
            Vec::new(),
            16_000,
            None,
            &tx,
        )
        .await;

        let transcript = rx.try_recv().expect("transcript should be sent");
        assert_eq!(transcript.text, "hello there");
        assert_eq!(transcript.start_ms, 2000);
        assert_eq!(transcript.end_ms, 3000);
    }

    #[tokio::test]
    async fn emit_transcript_publishes_transcribed_audio_clip() {
        let (tx, _rx) = mpsc::channel(1);
        let bus = TopicBus::new(4);
        let stream = bus.subscribe(Topic::Sensation);
        tokio::pin!(stream);

        emit_transcript(
            vec![make_segment("hello", 0.0, 0.5)],
            Utc::now(),
            0.0,
            b"wav".to_vec(),
            16_000,
            Some(&bus),
            &tx,
        )
        .await;

        let payload = stream.next().await.expect("audio sensation should publish");
        let sensation = payload
            .downcast_ref::<Sensation>()
            .expect("payload should be a sensation");
        let Sensation::Of { payload, .. } = sensation else {
            panic!("expected audio clip sensation");
        };
        let audio = payload
            .downcast_ref::<AudioClip>()
            .expect("sensation should contain an audio clip");
        assert_eq!(audio.transcript.as_deref(), Some("hello"));
        assert_eq!(audio.sample_rate, 16_000);
        assert_eq!(audio.channels, 1);
    }
}

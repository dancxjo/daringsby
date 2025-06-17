#![cfg(feature = "tts")]
use async_trait::async_trait;
use pragmatic_segmenter::Segmenter;
use psyche::{Event, Mouth};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::broadcast;
use tracing::error;

use anyhow::Result;
use base64::{Engine as _, engine::general_purpose};
use natural_tts::{
    Model, NaturalTtsBuilder,
    models::msedge::{MSEdgeModel, SpeechConfig},
};
use std::sync::Mutex;

pub trait Tts: Send + Sync {
    fn to_wav(&self, text: &str) -> Result<Vec<u8>>;
}

pub struct EdgeTts {
    inner: Mutex<natural_tts::NaturalTts>,
}

impl EdgeTts {
    pub fn new() -> Self {
        let cfg = SpeechConfig {
            voice_name: "en-US-AnnaNeural".to_string(),
            audio_format: "riff-24khz-16bit-mono-pcm".to_string(),
            pitch: 0,
            rate: 0,
            volume: 0,
        };
        let model = MSEdgeModel::new(cfg);
        let tts = NaturalTtsBuilder::default()
            .msedge_model(model)
            .default_model(Model::MSEdge)
            .build()
            .expect("construct msedge tts");
        Self {
            inner: Mutex::new(tts),
        }
    }
}

impl Tts for EdgeTts {
    fn to_wav(&self, text: &str) -> Result<Vec<u8>> {
        let mut tts = self.inner.lock().unwrap();
        let audio = tts.synthesize_auto(text.to_string())?;
        let rate = match audio.spec {
            natural_tts::models::Spec::Wav(spec) => spec.sample_rate,
            _ => 24_000,
        };
        Ok(samples_to_wav_bytes(&audio.data, rate))
    }
}

fn samples_to_wav_bytes(data: &[f32], sample_rate: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(44 + data.len() * 4);
    out.extend_from_slice(b"RIFF");
    let chunk_size = 36 + data.len() * 4;
    out.extend(&(chunk_size as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend(&(16u32.to_le_bytes()));
    out.extend(&(1u16).to_le_bytes()); // PCM
    out.extend(&(1u16).to_le_bytes()); // mono
    out.extend(&sample_rate.to_le_bytes());
    out.extend(&(sample_rate * 4).to_le_bytes());
    out.extend(&(4u16).to_le_bytes());
    out.extend(&(32u16).to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend(&((data.len() * 4) as u32).to_le_bytes());
    for s in data {
        let i = (s * 2147483647.0) as i32;
        out.extend(&i.to_le_bytes());
    }
    out
}

#[derive(Clone)]
pub struct TtsMouth {
    events: broadcast::Sender<Event>,
    speaking: Arc<AtomicBool>,
    tts: Arc<dyn Tts>,
}

impl TtsMouth {
    pub fn new(
        events: broadcast::Sender<Event>,
        speaking: Arc<AtomicBool>,
        tts: Arc<dyn Tts>,
    ) -> Self {
        Self {
            events,
            speaking,
            tts,
        }
    }
}

#[async_trait]
impl Mouth for TtsMouth {
    async fn speak(&self, text: &str) {
        self.speaking.store(true, Ordering::SeqCst);
        let seg = Segmenter::new().expect("segmenter init");
        for sentence in seg.segment(text) {
            let sent = sentence.trim();
            if sent.is_empty() {
                continue;
            }
            match self.tts.to_wav(sent) {
                Ok(bytes) => {
                    let b64 = general_purpose::STANDARD.encode(bytes);
                    if self.events.send(Event::SpeechAudio(b64)).is_err() {
                        error!("failed sending audio");
                    }
                }
                Err(e) => {
                    error!(?e, "tts failed");
                }
            }
        }
        self.speaking.store(false, Ordering::SeqCst);
    }

    async fn interrupt(&self) {
        self.speaking.store(false, Ordering::SeqCst);
    }

    fn speaking(&self) -> bool {
        self.speaking.load(Ordering::SeqCst)
    }
}

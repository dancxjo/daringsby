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
use pyo3::IntoPy;

pub trait Tts: Send + Sync {
    fn to_wav(&self, text: &str) -> Result<Vec<u8>>;
}

pub struct CoquiTts {
    inner: pyo3::Py<pyo3::PyAny>,
}

impl CoquiTts {
    pub fn new() -> Result<Self> {
        use pyo3::types::{PyDict, PyModule};
        pyo3::Python::with_gil(|py| {
            let module = PyModule::import(py, "TTS.api")?;
            let class = module.getattr("TTS")?;
            let kwargs = PyDict::new(py);
            kwargs.set_item("progress_bar", false)?;
            let obj = class.call((), Some(kwargs))?;
            Ok(Self {
                inner: obj.into_py(py),
            })
        })
    }
}

impl Tts for CoquiTts {
    fn to_wav(&self, text: &str) -> Result<Vec<u8>> {
        use std::fs;
        pyo3::Python::with_gil(|py| {
            let tts = self.inner.as_ref(py);
            let path = "/tmp/pete_tts.wav";
            tts.call_method1("tts_to_file", (text, path))?;
            let data = fs::read(path)?;
            let _ = fs::remove_file(path);
            Ok(data)
        })
    }
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

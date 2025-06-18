#![cfg(feature = "tts")]
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use pragmatic_segmenter::Segmenter;
use psyche::{Event, Mouth};
use std::pin::Pin;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::broadcast;
use tracing::{error, info};

use anyhow::Result;
use base64::{Engine as _, engine::general_purpose};
use reqwest::{Client, Url};
use urlencoding::encode;

/// Stream of raw WAV data chunks.
pub type TtsStream = Pin<Box<dyn Stream<Item = Result<Vec<u8>>> + Send>>;

/// Text-to-speech engine interface.
#[async_trait]
pub trait Tts: Send + Sync {
    /// Return a stream of WAV bytes for `text`.
    async fn stream_wav(&self, text: &str) -> Result<TtsStream>;
}

/// Client for a Coqui TTS server.
#[derive(Clone)]
pub struct CoquiTts {
    url: String,
    client: Client,
    speaker_id: Option<String>,
    language_id: Option<String>,
}

impl CoquiTts {
    /// Create a new client targeting `url` (e.g. `http://localhost:5002/api/tts`).
    ///
    /// Optional `speaker_id` and `language_id` parameters allow selecting a
    /// specific voice and language on the TTS server.
    pub fn new(
        url: impl Into<String>,
        speaker_id: Option<String>,
        language_id: Option<String>,
    ) -> Self {
        Self {
            url: url.into(),
            client: Client::new(),
            speaker_id,
            language_id,
        }
    }
}

#[async_trait]
impl Tts for CoquiTts {
    async fn stream_wav(&self, text: &str) -> Result<TtsStream> {
        let mut url = Url::parse(&self.url)?;
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("text", &encode(text).to_string());
            // Always include speaker_id and language_id, using defaults if not provided
            qp.append_pair("speaker_id", self.speaker_id.as_deref().unwrap_or("p340"));
            qp.append_pair("language_id", self.language_id.as_deref().unwrap_or(""));
            if let Some(ref l) = self.language_id {
                qp.append_pair("language_id", l);
            }
        }
        info!(%url, "requesting TTS");
        let resp = self.client.get(url).send().await?;
        let stream = resp
            .bytes_stream()
            .map(|b| b.map(|bytes| bytes.to_vec()).map_err(|e| e.into()));
        Ok(Box::pin(stream))
    }
}

/// [`Mouth`] implementation that streams audio via [`Tts`] and forwards it as
/// [`Event::SpeechAudio`] chunks.
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
            match self.tts.stream_wav(sent).await {
                Ok(mut stream) => {
                    let mut buf = Vec::new();
                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(bytes) if !bytes.is_empty() => buf.extend(bytes),
                            Ok(_) => {}
                            Err(e) => {
                                error!(?e, "tts streaming failed");
                                break;
                            }
                        }
                    }
                    if !buf.is_empty() {
                        let b64 = general_purpose::STANDARD.encode(buf);
                        if self.events.send(Event::SpeechAudio(b64)).is_err() {
                            error!("failed sending audio chunk");
                        }
                    }
                }
                Err(e) => {
                    error!(?e, "tts request failed");
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

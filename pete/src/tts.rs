use async_trait::async_trait;
use futures::StreamExt;
use lingproc::segment_text_into_sentences;
use psyche::{Event, PlainMouth, traits::Mouth};
#[cfg(feature = "tts")]
use psyche::traits::{Tts, TtsStream};
use crate::{ChannelMouth, EventBus};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::broadcast;
use tracing::{error, info};

use anyhow::Result;
use base64::{Engine as _, engine::general_purpose};
use reqwest::{Client, Url};

/// Client for a Coqui TTS server.
#[cfg_attr(feature = "tts", derive(Clone))]
#[cfg(feature = "tts")]
pub struct CoquiTts {
    url: String,
    client: Client,
    speaker_id: Option<String>,
    /// Optional language code passed as the `language_id` query parameter
    language_id: Option<String>,
}

#[cfg(feature = "tts")]
impl CoquiTts {
    /// Create a new client targeting `url` (e.g. `http://localhost:5002/api/tts`).
    ///
    /// Optional `speaker_id` selects the voice. `language_id` is passed as the
    /// corresponding query parameter so audio is produced in the desired
    /// language.
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
#[cfg(feature = "tts")]
impl Tts for CoquiTts {
    async fn stream_wav(&self, text: &str) -> Result<TtsStream> {
        let mut url = Url::parse(&self.url)?;
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("text", text);
            // Always include speaker_id, style_wav and language_id parameters
            // providing defaults when values are not configured
            qp.append_pair("speaker_id", self.speaker_id.as_deref().unwrap_or("p123"));
            qp.append_pair("style_wav", "");
            qp.append_pair("language_id", self.language_id.as_deref().unwrap_or(""));
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
/// [`Event::Speech`] chunks.
#[derive(Clone)]
#[cfg(feature = "tts")]
pub struct TtsMouth {
    events: broadcast::Sender<Event>,
    speaking: Arc<AtomicBool>,
    tts: Arc<dyn Tts>,
}

#[cfg(feature = "tts")]
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
#[cfg(feature = "tts")]
impl Mouth for TtsMouth {
    async fn speak(&self, text: &str) {
        self.speaking.store(true, Ordering::SeqCst);
        for sentence in segment_text_into_sentences(text) {
            let sent = sentence.trim();
            if sent.is_empty() {
                continue;
            }
            let (clean, _emo) = psyche::extract_emojis(sent);
            match self.tts.stream_wav(&clean).await {
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
                        if self
                            .events
                            .send(Event::Speech {
                                text: sent.to_string(),
                                audio: Some(b64),
                            })
                            .is_err()
                        {
                            error!("failed sending speech chunk");
                        }
                    } else {
                        let _ = self.events.send(Event::Speech {
                            text: sent.to_string(),
                            audio: None,
                        });
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

/// Create the mouth implementation used by the application.
///
/// When the `tts` feature is enabled this wraps a [`TtsMouth`] with
/// [`PlainMouth`] so Markdown formatting is stripped before speaking.
/// Otherwise a [`ChannelMouth`] that emits text-only speech events is returned.
pub fn default_mouth(
    bus: Arc<EventBus>,
    speaking: Arc<AtomicBool>,
    tts_url: String,
    speaker_id: Option<String>,
    language_id: Option<String>,
) -> Arc<dyn Mouth> {
    #[cfg(feature = "tts")]
    {
        let tts = Arc::new(CoquiTts::new(tts_url, speaker_id, language_id)) as Arc<dyn Tts>;
        let mouth = Arc::new(TtsMouth::new(
            bus.event_sender(),
            speaking.clone(),
            tts,
        )) as Arc<dyn Mouth>;
        return Arc::new(PlainMouth::new(mouth)) as Arc<dyn Mouth>;
    }
    #[cfg(not(feature = "tts"))]
    {
        let _ = (tts_url, speaker_id, language_id);
        Arc::new(ChannelMouth::new(bus, speaking)) as Arc<dyn Mouth>
    }
}

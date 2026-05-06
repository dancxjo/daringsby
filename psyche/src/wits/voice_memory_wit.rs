use crate::traits::observer::SensationObserver;
use crate::traits::wit::Wit;
use crate::{Impression, Sensation, Stimulus, VoiceInfo, audio_captured_at};
use async_trait::async_trait;
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::broadcast;
use tracing::info;

/// Wit that notes when familiar or new voices are heard.
pub struct VoiceMemoryWit {
    buffer: Mutex<Vec<Stimulus<VoiceInfo>>>,
    last_voice: Mutex<Option<Vec<f32>>>,
    ticks_without_voice: AtomicUsize,
    tx: Option<broadcast::Sender<crate::WitReport>>,
}

impl VoiceMemoryWit {
    /// Debug label.
    pub const LABEL: &'static str = "VoiceMemory";

    /// Create a new `VoiceMemoryWit`.
    pub fn new() -> Self {
        Self {
            buffer: Mutex::new(Vec::new()),
            last_voice: Mutex::new(None),
            ticks_without_voice: AtomicUsize::new(0),
            tx: None,
        }
    }

    /// Emit [`WitReport`]s using `tx`.
    pub fn with_debug(tx: broadcast::Sender<crate::WitReport>) -> Self {
        Self {
            tx: Some(tx),
            ..Self::new()
        }
    }
}

fn similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (norm_a * norm_b + 1e-5)
}

#[async_trait]
impl Wit for VoiceMemoryWit {
    type Input = VoiceInfo;
    type Output = VoiceInfo;

    async fn observe(&self, info: Self::Input) {
        let timestamp = audio_captured_at(&info.clip).unwrap_or_else(chrono::Utc::now);
        self.buffer.lock().unwrap().push(Stimulus {
            what: info,
            timestamp,
            source_sensation_ids: Vec::new(),
        });
    }

    async fn tick(&self) -> Vec<Impression<Self::Output>> {
        let items = {
            let mut buf = self.buffer.lock().unwrap();
            if buf.is_empty() {
                self.ticks_without_voice.fetch_add(1, Ordering::SeqCst);
                return Vec::new();
            }
            self.ticks_without_voice.store(0, Ordering::SeqCst);
            buf.drain(..).collect::<Vec<_>>()
        };

        let mut out = Vec::new();
        for item in items {
            let info = item.what;
            let summary;
            {
                let mut last = self.last_voice.lock().unwrap();
                summary = if let Some(prev) = last.as_ref() {
                    if similarity(prev, &info.embedding) > 0.9 {
                        "I heard the same voice again.".to_string()
                    } else {
                        "I think I heard a new voice.".to_string()
                    }
                } else {
                    "I think I heard a new voice.".to_string()
                };
                *last = Some(info.embedding.clone());
            }
            info!(%summary, "voice memory observation");
            if let Some(tx) = &self.tx {
                if crate::debug::debug_enabled(Self::LABEL).await {
                    let _ = tx.send(crate::WitReport {
                        name: Self::LABEL.into(),
                        prompt: "voice memory".into(),
                        output: summary.clone(),
                    });
                }
            }
            out.push(Impression::new(
                vec![Stimulus {
                    what: info.clone(),
                    timestamp: item.timestamp,
                    source_sensation_ids: item.source_sensation_ids,
                }],
                summary,
                None::<String>,
            ));
        }
        out
    }

    fn debug_label(&self) -> &'static str {
        Self::LABEL
    }
}

#[async_trait]
impl SensationObserver for VoiceMemoryWit {
    async fn observe_sensation(&self, payload: &(dyn std::any::Any + Send + Sync)) {
        if let Some(s) = payload.downcast_ref::<Sensation>() {
            if let Sensation::Of {
                payload,
                occurred_at,
            } = s
            {
                if let Some(info) = payload.downcast_ref::<VoiceInfo>() {
                    self.buffer.lock().unwrap().push(Stimulus {
                        what: info.clone(),
                        timestamp: *occurred_at,
                        source_sensation_ids: vec![s.id()],
                    });
                }
            }
        }
    }
}

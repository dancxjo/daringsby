use crate::sensors::face::FaceInfo;
use crate::traits::observer::SensationObserver;
use crate::traits::wit::Wit;
use crate::{Impression, Sensation, Stimulus};
use async_trait::async_trait;
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::broadcast;
use tracing::info;

/// Wit that notes when familiar or new faces appear.
pub struct FaceMemoryWit {
    buffer: Mutex<Vec<FaceInfo>>,
    last_face: Mutex<Option<Vec<f32>>>,
    ticks_without_face: AtomicUsize,
    tx: Option<broadcast::Sender<crate::WitReport>>,
}

impl FaceMemoryWit {
    /// Debug label.
    pub const LABEL: &'static str = "FaceMemory";

    /// Create a new `FaceMemoryWit`.
    pub fn new() -> Self {
        Self {
            buffer: Mutex::new(Vec::new()),
            last_face: Mutex::new(None),
            ticks_without_face: AtomicUsize::new(0),
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
impl Wit<FaceInfo, FaceInfo> for FaceMemoryWit {
    async fn observe(&self, info: FaceInfo) {
        self.buffer.lock().unwrap().push(info);
    }

    async fn tick(&self) -> Vec<Impression<FaceInfo>> {
        let items = {
            let mut buf = self.buffer.lock().unwrap();
            if buf.is_empty() {
                self.ticks_without_face.fetch_add(1, Ordering::SeqCst);
                if self.ticks_without_face.load(Ordering::SeqCst) >= 5 {
                    self.ticks_without_face.store(0, Ordering::SeqCst);
                    info!("no faces detected for a while");
                    return vec![Impression::new(
                        vec![],
                        "No faces detected for a while now.",
                        None::<String>,
                    )];
                }
                return Vec::new();
            }
            self.ticks_without_face.store(0, Ordering::SeqCst);
            buf.drain(..).collect::<Vec<_>>()
        };

        let mut out = Vec::new();
        for info in items {
            let summary;
            {
                let mut last = self.last_face.lock().unwrap();
                summary = if let Some(prev) = last.as_ref() {
                    if similarity(prev, &info.embedding) > 0.9 {
                        "I saw the same person again.".to_string()
                    } else {
                        "I think someone new just showed up.".to_string()
                    }
                } else {
                    "I think someone new just showed up.".to_string()
                };
                *last = Some(info.embedding.clone());
            }
            info!(%summary, "face memory observation");
            if let Some(tx) = &self.tx {
                if crate::debug::debug_enabled(Self::LABEL).await {
                    let _ = tx.send(crate::WitReport {
                        name: Self::LABEL.into(),
                        prompt: "face memory".into(),
                        output: summary.clone(),
                    });
                }
            }
            out.push(Impression::new(
                vec![Stimulus::new(info.clone())],
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
impl SensationObserver for FaceMemoryWit {
    async fn observe_sensation(&self, payload: &(dyn std::any::Any + Send + Sync)) {
        if let Some(s) = payload.downcast_ref::<Sensation>() {
            if let Sensation::Of(any) = s {
                if let Some(info) = any.downcast_ref::<FaceInfo>() {
                    self.observe(info.clone()).await;
                }
            }
        }
    }
}

use crate::ling::{Doer, Instruction};
use crate::topics::{Topic, TopicBus};
use crate::{Impression, Sensation, Stimulus};
use async_trait::async_trait;
use futures::StreamExt;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tracing::debug;

/// Wit that groups simultaneous sensations into a single [`Impression`].
///
/// `InstantWit` listens on [`Topic::Sensation`] and buffers raw
/// [`Sensation`]s. On [`tick`], it summarizes the collected items using
/// the provided [`Doer`] and publishes the resulting impression on
/// [`Topic::Instant`].
pub struct InstantWit {
    buffer: Arc<Mutex<Vec<Arc<Sensation>>>>,
    bus: TopicBus,
    doer: Arc<dyn Doer>,
    tx: Option<broadcast::Sender<crate::WitReport>>, // optional debug
}

impl InstantWit {
    /// Debug label for this wit.
    pub const LABEL: &'static str = "InstantWit";

    /// Create a new `InstantWit` subscribed to `bus` using `doer`.
    pub fn new(bus: TopicBus, doer: Arc<dyn Doer>) -> Self {
        Self::with_debug(bus, doer, None)
    }

    /// Create a new `InstantWit` emitting [`WitReport`]s using `tx`.
    pub fn with_debug(
        bus: TopicBus,
        doer: Arc<dyn Doer>,
        tx: Option<broadcast::Sender<crate::WitReport>>,
    ) -> Self {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let buf_clone = buffer.clone();
        let bus_clone = bus.clone();
        tokio::spawn(async move {
            let mut stream = bus_clone.subscribe(Topic::Sensation);
            tokio::pin!(stream);
            while let Some(payload) = stream.next().await {
                if let Ok(s) = Arc::downcast::<Sensation>(payload) {
                    buf_clone.lock().unwrap().push(s);
                }
            }
        });
        Self {
            buffer,
            bus,
            doer,
            tx,
        }
    }

    /// Describe a sensation for the summarization prompt.
    fn describe(s: &Sensation) -> String {
        match s {
            Sensation::HeardOwnVoice(t) => format!("Pete said \"{}\"", t),
            Sensation::HeardUserVoice(t) => format!("User said \"{}\"", t),
            Sensation::Of(any) => {
                if let Some(_f) = any.downcast_ref::<crate::sensors::face::FaceInfo>() {
                    "Saw a face".to_string()
                } else if let Some(loc) = any.downcast_ref::<crate::GeoLoc>() {
                    format!(
                        "Detected location ({:.1}, {:.1})",
                        loc.latitude, loc.longitude
                    )
                } else {
                    "Something happened".to_string()
                }
            }
        }
    }
}

#[async_trait]
impl crate::traits::wit::Wit<(), String> for InstantWit {
    async fn observe(&self, _: ()) {}

    async fn tick(&self) -> Vec<Impression<String>> {
        let items = {
            let mut buf = self.buffer.lock().unwrap();
            if buf.is_empty() {
                return Vec::new();
            }
            let data = buf.drain(..).collect::<Vec<_>>();
            data
        };
        debug!(count = items.len(), "instant wit summarizing sensations");
        let bullets: Vec<String> = items.iter().map(|s| Self::describe(&*s)).collect();
        let prompt = format!(
            "Summarize these simultaneous sensations in one sentence:\n- {}",
            bullets.join("\n- ")
        );
        let out = match self
            .doer
            .follow(Instruction {
                command: prompt,
                images: Vec::new(),
            })
            .await
        {
            Ok(s) => s.trim().to_string(),
            Err(_) => String::new(),
        };
        let stim = Stimulus::new(out.clone());
        let imp = Impression::new(vec![stim], out.clone(), None::<String>);
        if let Some(tx) = &self.tx {
            if crate::debug::debug_enabled(Self::LABEL).await {
                let _ = tx.send(crate::WitReport {
                    name: Self::LABEL.into(),
                    prompt: "instant summary".into(),
                    output: out.clone(),
                });
            }
        }
        self.bus.publish(Topic::Instant, imp.clone());
        vec![imp]
    }

    fn debug_label(&self) -> &'static str {
        Self::LABEL
    }
}

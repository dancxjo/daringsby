//! The Quick — Pete's reflexive sensorium.
//!
//! The Quick receives a sequence of [`Sensation`]s and emits an [`Instant`] —
//! a one-sentence summary of what Pete just experienced. It is the first stage
//! of cognition, forming a perceptual narrative from raw sensory data.
//!
//! Example: "I'm seeing a fly quickly approach me and then hesitate."
//!
//! `Quick` listens on [`Topic::Sensation`] and buffers sensations over a short
//! window. On [`tick`], it condenses the recent sensations into a single
//! [`Instant`] and publishes it on [`Topic::Instant`].

use crate::topics::{Topic, TopicBus};
use crate::traits::{Doer, wit::Wit};
use crate::{Impression, Instant, Sensation, Stimulus};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use futures::StreamExt;
use lingproc::LlmInstruction;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tracing::{debug, info};
pub struct Quick {
    buffer: Arc<Mutex<VecDeque<(DateTime<Utc>, Arc<Sensation>)>>>,
    bus: TopicBus,
    doer: Arc<dyn Doer>,
    window: Duration,
    tx: Option<broadcast::Sender<crate::WitReport>>, // optional debug
}

impl Quick {
    /// Debug label for this wit.
    pub const LABEL: &'static str = "Quick";

    /// Create a new `Quick` subscribed to `bus` using `doer`.
    pub fn new(bus: TopicBus, doer: Arc<dyn Doer>) -> Self {
        Self::with_debug(bus, doer, None)
    }

    /// Create a new `Quick` emitting [`WitReport`]s using `tx`.
    pub fn with_debug(
        bus: TopicBus,
        doer: Arc<dyn Doer>,
        tx: Option<broadcast::Sender<crate::WitReport>>,
    ) -> Self {
        let buffer = Arc::new(Mutex::new(VecDeque::new()));
        let buf_clone = buffer.clone();
        let bus_clone = bus.clone();
        tokio::spawn(async move {
            let stream = bus_clone.subscribe(Topic::Sensation);
            tokio::pin!(stream);
            while let Some(payload) = stream.next().await {
                if let Ok(s) = Arc::downcast::<Sensation>(payload) {
                    let mut buf = buf_clone.lock().unwrap();
                    buf.push_back((Utc::now(), s));
                }
            }
        });
        Self {
            buffer,
            bus,
            doer,
            window: Duration::seconds(8),
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
                    debug!("unrecognized sensation type: {:?}", any.type_id());
                    "Something happened".to_string()
                }
            }
        }
    }

    /// Remove sensations older than the window from `buf`.
    fn trim_old(buf: &mut VecDeque<(DateTime<Utc>, Arc<Sensation>)>, window: Duration) {
        let cutoff = Utc::now() - window;
        while let Some((t, _)) = buf.front() {
            if *t < cutoff {
                buf.pop_front();
            } else {
                break;
            }
        }
    }
}

#[async_trait]
impl crate::traits::observer::SensationObserver for Quick {
    async fn observe_sensation(&self, payload: &(dyn std::any::Any + Send + Sync)) {
        if let Some(s) = payload.downcast_ref::<Sensation>() {
            self.observe(s.clone()).await;
        }
    }
}

#[async_trait]
impl crate::traits::wit::Wit for Quick {
    type Input = Sensation;
    type Output = Instant;

    async fn observe(&self, input: Self::Input) {
        let mut buf = self.buffer.lock().unwrap();
        buf.push_back((Utc::now(), Arc::new(input)));
        Self::trim_old(&mut buf, self.window);
    }

    async fn tick(&self) -> Vec<Impression<Self::Output>> {
        let items = {
            let mut buf = self.buffer.lock().unwrap();
            Self::trim_old(&mut buf, self.window);
            if buf.is_empty() {
                return Vec::new();
            }
            buf.drain(..).map(|(_, s)| s).collect::<Vec<_>>()
        };
        debug!(count = items.len(), "quick summarizing sensations");
        let bullets: Vec<String> = items.iter().map(|s| Self::describe(&*s)).collect();
        let prompt = format!(
            "Summarize these simultaneous sensations in one sentence:\n- {}",
            bullets.join("\n- ")
        );
        let out = match self
            .doer
            .follow(LlmInstruction {
                command: prompt,
                images: Vec::new(),
            })
            .await
        {
            Ok(s) => s.trim().to_string(),
            Err(_) => String::new(),
        };
        let instant = Instant {
            at: Utc::now(),
            sensations: items.clone(),
        };
        let stim = Stimulus::new(instant);
        info!(count = items.len(), out = %out, "quick emitting instant");
        debug!(
            "quick: emitting instant from {} sensations: \"{}\"",
            items.len(),
            out
        );
        let imp = Impression::new(vec![stim], out.clone(), None::<String>);
        if let Some(tx) = &self.tx {
            if crate::debug::debug_enabled(Self::LABEL).await {
                let _ = tx.send(crate::WitReport {
                    name: Self::LABEL.into(),
                    prompt: "quick summary".into(),
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

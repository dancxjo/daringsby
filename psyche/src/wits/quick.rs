//! The Quick — Pete's reflexive sensorium.
//!
//! The Quick receives a sequence of [`Sensation`]s and emits an
//! [`Impression<String>`] with timestamped [`Stimulus`] entries. It is the
//! first stage of cognition, forming a perceptual narrative from raw sensory
//! data.
//!
//! Example: "I'm seeing a fly quickly approach me and then hesitate."
//!
//! `Quick` listens on [`Topic::Sensation`] and buffers sensations over a short
//! window. On [`tick`], it condenses the recent sensations into a single
//! impression and publishes it on [`Topic::Instant`].

use crate::topics::{Topic, TopicBus};
use crate::traits::Doer;
use crate::{Impression, Sensation, Stimulus};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use futures::StreamExt;
use lingproc::LlmInstruction;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tracing::{debug, trace};
pub struct Quick {
    buffer: Arc<Mutex<VecDeque<Stimulus<String>>>>,
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
                    if let Some(description) = Quick::describe(&s) {
                        let mut buf = buf_clone.lock().unwrap();
                        buf.push_back(Stimulus::with_source_sensation_ids(
                            description,
                            s.occurred_at(),
                            [s.id()],
                        ));
                    }
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
    fn describe(s: &Sensation) -> Option<String> {
        match s {
            Sensation::HeardOwnVoice { text, .. } => Some(format!("I said \"{}\"", text)),
            Sensation::HeardUserVoice { text, .. } => Some(format!("User said \"{}\"", text)),
            Sensation::WebInterfaceText { text, .. } => Some(format!(
                "I hear someone on my web interface type: {}",
                text.trim()
            )),
            Sensation::StartedSpeaking { text, .. } => {
                Some(format!("I start saying \"{}\"", text.trim()))
            }
            Sensation::FinishedSpeaking { text, .. } => {
                Some(format!("I finish saying \"{}\"", text.trim()))
            }
            Sensation::Of { payload, .. } => {
                if let Some(_f) = payload.downcast_ref::<crate::sensors::face::FaceInfo>() {
                    Some(crate::prompt::face_count_sensation_text(1))
                } else if payload.downcast_ref::<crate::ImageEmbedding>().is_some() {
                    Some("I recognized the whole camera frame visually".to_string())
                } else if payload.downcast_ref::<crate::GeoEmbedding>().is_some() {
                    Some("I recognized where I am".to_string())
                } else if payload.downcast_ref::<crate::VoiceInfo>().is_some() {
                    Some("I heard a voice".to_string())
                } else if let Some(summary) = payload.downcast_ref::<crate::CombobulationSummary>()
                {
                    Some(format!(
                        "A prior combobulation summary said \"{}\"",
                        summary.text
                    ))
                } else if let Some(impression) = payload.downcast_ref::<crate::Impression<String>>()
                {
                    Some(impression.summary.clone())
                } else if payload.downcast_ref::<crate::AudioClip>().is_some() {
                    None
                } else if payload.downcast_ref::<crate::ImageData>().is_some() {
                    None
                } else if let Some(loc) = payload.downcast_ref::<crate::GeoLoc>() {
                    Some(format!(
                        "I detected location ({:.1}, {:.1})",
                        loc.latitude, loc.longitude
                    ))
                } else if let Some(motion) = payload.downcast_ref::<crate::BrowserMotion>() {
                    if let Some(accel) = motion
                        .acceleration
                        .as_ref()
                        .or(motion.acceleration_including_gravity.as_ref())
                    {
                        Some(format!(
                            "I felt device motion acceleration ({:.2}, {:.2}, {:.2})",
                            accel.x.unwrap_or_default(),
                            accel.y.unwrap_or_default(),
                            accel.z.unwrap_or_default()
                        ))
                    } else if let Some(orientation) = &motion.orientation {
                        Some(format!(
                            "I felt the device orientation shift ({:.1}, {:.1}, {:.1})",
                            orientation.alpha.unwrap_or_default(),
                            orientation.beta.unwrap_or_default(),
                            orientation.gamma.unwrap_or_default()
                        ))
                    } else {
                        Some("I felt browser device motion".to_string())
                    }
                } else if let Some(beat) = payload.downcast_ref::<crate::Heartbeat>() {
                    Some(format!("I felt a heartbeat at {}", beat.timestamp))
                } else {
                    trace!("unrecognized sensation type: {:?}", payload.type_id());
                    Some("I sensed something happened".to_string())
                }
            }
        }
    }

    /// Remove sensations older than the window from `buf`.
    fn trim_old(buf: &mut VecDeque<Stimulus<String>>, window: Duration) {
        let cutoff = Utc::now() - window;
        while let Some(stimulus) = buf.front() {
            if stimulus.timestamp < cutoff {
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
            if let Some(description) = Self::describe(s) {
                let mut buf = self.buffer.lock().unwrap();
                buf.push_back(Stimulus::with_source_sensation_ids(
                    description,
                    s.occurred_at(),
                    [s.id()],
                ));
                Self::trim_old(&mut buf, self.window);
            }
        }
    }
}

#[async_trait]
impl crate::traits::wit::Wit for Quick {
    type Input = Sensation;
    type Output = String;

    async fn observe(&self, input: Self::Input) {
        if let Some(description) = Self::describe(&input) {
            let mut buf = self.buffer.lock().unwrap();
            buf.push_back(Stimulus::with_source_sensation_ids(
                description,
                input.occurred_at(),
                [input.id()],
            ));
            Self::trim_old(&mut buf, self.window);
        }
    }

    async fn tick(&self) -> Vec<Impression<Self::Output>> {
        let items = {
            let mut buf = self.buffer.lock().unwrap();
            Self::trim_old(&mut buf, self.window);
            if buf.is_empty() {
                return Vec::new();
            }
            buf.drain(..).collect::<Vec<_>>()
        };
        trace!(count = items.len(), "quick summarizing sensations");
        let stimuli = items;
        let prompt_bullets: Vec<String> = stimuli.iter().map(Stimulus::prompt_list_item).collect();
        let fallback_bullets: Vec<String> = stimuli.iter().map(|s| s.what.clone()).collect();
        let grounding = crate::prompt::SENSOR_GROUNDING_RULES;
        let prompt = format!(
            "Summarize these recent sensations in one short sentence, in the first person, using I/my/me. Try to infer what is happening in the real world from fragmentary, possibly contradictory, fleeting sensory data. Some sensations may be consecutive frames from the same sensor stream; repeated similar camera or face observations usually mean one thing persisted across frames, not multiple simultaneous things. {grounding} Compress repeated low-level detections into the real-world gist; do not list ids, hashes, timestamps, or detection-by-detection details. Do not refer to Pete, the individual, the observer, or the person. Return only the summary sentence.\n- {}",
            prompt_bullets.join("\n- ")
        );
        let command = crate::with_default_system_prompt(prompt);
        let out = match self
            .doer
            .follow(LlmInstruction {
                command: command.clone(),
                images: Vec::new(),
            })
            .await
        {
            Ok(s) => {
                let summary = s.trim();
                if summary.is_empty() {
                    fallback_bullets.join("; ")
                } else {
                    summary.to_string()
                }
            }
            Err(_) => fallback_bullets.join("; "),
        };
        debug!(count = stimuli.len(), summary = %out, "quick emitting instant impression");
        trace!(
            "quick: emitting instant impression from {} sensations: \"{}\"",
            stimuli.len(),
            out
        );
        let imp = Impression::new(stimuli, out.clone(), None::<String>);
        if let Some(tx) = &self.tx {
            if crate::debug::debug_enabled(Self::LABEL).await {
                let _ = tx.send(crate::WitReport {
                    name: Self::LABEL.into(),
                    prompt: command,
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

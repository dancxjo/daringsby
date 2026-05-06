use crate::prompt::PromptFragment;
use crate::topics::{Topic, TopicBus};
use crate::traits::Doer;
use crate::{CombobulationSummary, ImageData, Impression, Sensation, Stimulus, wit::Wit};
use async_trait::async_trait;
use chrono::Utc;
use futures::StreamExt;
use lingproc::LlmInstruction;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{Semaphore, broadcast};
use tracing::info;

#[cfg(not(test))]
const CAPTION_COOLDOWN: Duration = Duration::from_secs(30);
#[cfg(test)]
const CAPTION_COOLDOWN: Duration = Duration::from_secs(1);

/// Wit summarizing recent episodes into a short awareness statement.
pub struct Combobulator {
    doer: Arc<dyn Doer>,
    prompt: crate::prompt::CombobulatorPrompt,
    bus: Option<TopicBus>,
    tx: Option<broadcast::Sender<crate::WitReport>>,
    buffer: Arc<Mutex<Vec<Impression<String>>>>,
    last_caption_time: Mutex<Instant>,
    latest_image: Arc<Mutex<Option<ImageData>>>,
    llm_semaphore: Arc<Semaphore>,
}

impl Combobulator {
    /// Debug label for this Wit.
    pub const LABEL: &'static str = "Combobulator";
    /// Create a new `Combobulator` using the provided [`Doer`].
    pub fn new(doer: Arc<dyn Doer>) -> Self {
        Self::with_debug(doer, None)
    }

    /// Create a `Combobulator` that emits [`WitReport`]s using `tx`.
    pub fn with_debug(
        doer: Arc<dyn Doer>,
        tx: Option<broadcast::Sender<crate::WitReport>>,
    ) -> Self {
        Self::build(None, doer, tx)
    }

    /// Create a `Combobulator` subscribed to immediate impressions on `bus`.
    pub fn with_bus(bus: TopicBus, doer: Arc<dyn Doer>) -> Self {
        Self::with_bus_and_debug(bus, doer, None)
    }

    /// Create a bus-backed `Combobulator` that emits [`WitReport`]s using `tx`.
    pub fn with_bus_and_debug(
        bus: TopicBus,
        doer: Arc<dyn Doer>,
        tx: Option<broadcast::Sender<crate::WitReport>>,
    ) -> Self {
        Self::build(Some(bus), doer, tx)
    }

    fn build(
        bus: Option<TopicBus>,
        doer: Arc<dyn Doer>,
        tx: Option<broadcast::Sender<crate::WitReport>>,
    ) -> Self {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        if let Some(bus) = &bus {
            let buf_clone = buffer.clone();
            let bus_clone = bus.clone();
            tokio::spawn(async move {
                let stream = bus_clone.subscribe(Topic::Instant);
                tokio::pin!(stream);
                while let Some(payload) = stream.next().await {
                    if let Ok(i) = Arc::downcast::<Impression<String>>(payload) {
                        buf_clone.lock().unwrap().push((*i).clone());
                    }
                }
            });
        }
        Self {
            doer,
            prompt: crate::prompt::CombobulatorPrompt,
            bus,
            tx,
            buffer,
            last_caption_time: Mutex::new(Instant::now() - Duration::from_secs(30)),
            latest_image: Arc::new(Mutex::new(None)),
            llm_semaphore: Arc::new(Semaphore::new(2)),
        }
    }

    /// Replace the prompt builder.
    pub fn set_prompt(&mut self, prompt: crate::prompt::CombobulatorPrompt) {
        self.prompt = prompt;
    }
}

#[async_trait]
impl Wit for Combobulator {
    type Input = Impression<String>;
    type Output = String;

    async fn observe(&self, input: Self::Input) {
        self.buffer.lock().unwrap().push(input);
    }

    async fn tick(&self) -> Vec<Impression<Self::Output>> {
        let now = Instant::now();
        let image = {
            let mut last = self.last_caption_time.lock().unwrap();
            if now.duration_since(*last) < CAPTION_COOLDOWN {
                None
            } else {
                *last = now;
                self.latest_image.lock().unwrap().take()
            }
        };

        if let Some(image) = image {
            let permit = self.llm_semaphore.clone().acquire_owned().await.unwrap();
            let start = Instant::now();
            let result = self.describe_image(&image).await;
            info!("🖼️ image captioning took {:?}", start.elapsed());
            drop(permit);
            if let Ok(caption) = result {
                self.buffer.lock().unwrap().push(Impression::new(
                    vec![Stimulus::new(caption.clone())],
                    caption,
                    None::<String>,
                ));
            }
        }

        let inputs = {
            let mut buf = self.buffer.lock().unwrap();
            if buf.is_empty() {
                return Vec::new();
            }
            let data = buf.clone();
            buf.clear();
            data
        };
        match self.digest(&inputs).await {
            Ok(i) => vec![i],
            Err(_) => Vec::new(),
        }
    }

    fn debug_label(&self) -> &'static str {
        Self::LABEL
    }
}

impl Combobulator {
    /// Summarize `inputs` into a short awareness statement.
    pub async fn digest(
        &self,
        inputs: &[Impression<String>],
    ) -> anyhow::Result<Impression<String>> {
        let combined = inputs
            .iter()
            .filter_map(|imp| imp.stimuli.first().map(Stimulus::prompt_list_item))
            .collect::<Vec<_>>()
            .join("\n- ");
        let instruction = LlmInstruction {
            command: crate::with_default_system_prompt(
                self.prompt.build_prompt(&format!("- {combined}")),
            ),
            images: Vec::new(),
        };
        let resp = self.doer.follow(instruction.clone()).await?;
        let summary = resp.trim().to_string();
        if let Some(tx) = &self.tx {
            if crate::debug::debug_enabled(Self::LABEL).await {
                let _ = tx.send(crate::WitReport {
                    name: Self::LABEL.into(),
                    prompt: instruction.command.clone(),
                    output: summary.clone(),
                });
            }
        }
        let imp = Impression::new(
            vec![Stimulus::new(summary.clone())],
            summary.clone(),
            None::<String>,
        );
        if let Some(bus) = &self.bus {
            bus.publish(Topic::Moment, imp.clone());
            let now = Utc::now();
            bus.publish(
                Topic::Sensation,
                Sensation::of_at(
                    CombobulationSummary {
                        text: summary,
                        created_at: Some(now.to_rfc3339()),
                        source_sensation_ids: Vec::new(),
                    },
                    now,
                ),
            );
        }
        Ok(imp)
    }

    /// Describe an image using the underlying [`Doer`].
    pub async fn describe_image(&self, image: &crate::ImageData) -> anyhow::Result<String> {
        use lingproc::ImageData as LImageData;
        let caption = self
            .doer
            .follow(LlmInstruction {
                command: crate::with_default_system_prompt(crate::prompt::IMAGE_CAPTION_PROMPT),
                images: vec![LImageData {
                    mime: image.mime.clone(),
                    base64: image.base64.clone(),
                    captured_at: image.captured_at.clone(),
                }],
            })
            .await?;
        Ok(caption.trim().to_string())
    }

    /// Continuously tick the wit at a fixed interval.
    pub async fn run(self: Arc<Self>) {
        loop {
            self.tick().await;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}

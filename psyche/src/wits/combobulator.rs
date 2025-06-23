use crate::prompt::PromptBuilder;
use crate::traits::Doer;
use crate::{
    ImageData, Impression, Stimulus,
    wit::{Episode, Wit},
};
use async_trait::async_trait;
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
    tx: Option<broadcast::Sender<crate::WitReport>>,
    buffer: Mutex<Vec<Impression<Episode>>>,
    last_caption_time: Mutex<Instant>,
    latest_image: Arc<Mutex<Option<ImageData>>>,
    llm_semaphore: Arc<Semaphore>,
    instants: Mutex<Vec<Arc<crate::Instant>>>,
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
        Self {
            doer,
            prompt: crate::prompt::CombobulatorPrompt,
            tx,
            buffer: Mutex::new(Vec::new()),
            last_caption_time: Mutex::new(Instant::now() - Duration::from_secs(30)),
            latest_image: Arc::new(Mutex::new(None)),
            llm_semaphore: Arc::new(Semaphore::new(2)),
            instants: Mutex::new(Vec::new()),
        }
    }

    /// Replace the prompt builder.
    pub fn set_prompt(&mut self, prompt: crate::prompt::CombobulatorPrompt) {
        self.prompt = prompt;
    }
}

#[async_trait]
impl crate::traits::observer::SensationObserver for Combobulator {
    async fn observe_sensation(&self, payload: &(dyn std::any::Any + Send + Sync)) {
        if let Some(instant) = payload.downcast_ref::<Arc<crate::Instant>>() {
            self.instants.lock().unwrap().push(instant.clone());
        }
    }
}

#[async_trait]
impl Wit for Combobulator {
    type Input = Impression<Episode>;
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
                    vec![Stimulus::new(Episode {
                        summary: caption.clone(),
                    })],
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
        inputs: &[Impression<Episode>],
    ) -> anyhow::Result<Impression<String>> {
        let mut combined = String::new();
        for imp in inputs {
            if let Some(stim) = imp.stimuli.first() {
                if !combined.is_empty() {
                    combined.push(' ');
                }
                combined.push_str(&stim.what.summary);
            }
        }
        let instruction = LlmInstruction {
            command: self.prompt.build(&combined),
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
        Ok(Impression::new(
            vec![Stimulus::new(summary.clone())],
            summary,
            None::<String>,
        ))
    }

    /// Describe an image using the underlying [`Doer`].
    pub async fn describe_image(&self, image: &crate::ImageData) -> anyhow::Result<String> {
        use lingproc::ImageData as LImageData;
        let caption = self
            .doer
            .follow(LlmInstruction {
                command: "Describe only what you see in this image in a single sentence, in the first person. Remember, this is what you are *seeing* in the first person, so unless you're looking into a mirror, you won't be seeing yourself.".into(),
                images: vec![LImageData { mime: image.mime.clone(), base64: image.base64.clone() }],
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

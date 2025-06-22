use crate::{
    ImageData, Impression, Stimulus, Summarizer,
    wit::{Episode, Wit},
    wits::Combobulator,
};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tracing::info;

#[cfg(not(test))]
const CAPTION_COOLDOWN: Duration = Duration::from_secs(30);
#[cfg(test)]
const CAPTION_COOLDOWN: Duration = Duration::from_secs(1);

/// Wit summarizing recent episodes into a short awareness statement.
pub struct CombobulatorWit {
    combobulator: Combobulator,
    buffer: Mutex<Vec<Impression<Episode>>>,
    last_caption_time: Mutex<Instant>,
    latest_image: Arc<Mutex<Option<ImageData>>>,
    llm_semaphore: Arc<Semaphore>,
    instants: Mutex<Vec<Arc<crate::Instant>>>,
}

impl CombobulatorWit {
    /// Debug label for this Wit.
    pub const LABEL: &'static str = "CombobulatorWit";
    /// Create a new `CombobulatorWit` using the given summarizer.
    pub fn new(combobulator: Combobulator) -> Self {
        Self {
            combobulator,
            buffer: Mutex::new(Vec::new()),
            last_caption_time: Mutex::new(Instant::now() - Duration::from_secs(30)),
            latest_image: Arc::new(Mutex::new(None)),
            llm_semaphore: Arc::new(Semaphore::new(2)),
            instants: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl crate::traits::observer::SensationObserver for CombobulatorWit {
    async fn observe_sensation(&self, payload: &(dyn std::any::Any + Send + Sync)) {
        if let Some(instant) = payload.downcast_ref::<Arc<crate::Instant>>() {
            self.instants.lock().unwrap().push(instant.clone());
        }
    }
}

#[async_trait]
impl Wit<Impression<Episode>, String> for CombobulatorWit {
    async fn observe(&self, input: Impression<Episode>) {
        self.buffer.lock().unwrap().push(input);
    }

    async fn tick(&self) -> Vec<Impression<String>> {
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
            let result = self.combobulator.describe_image(&image).await;
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
        match self.combobulator.digest(&inputs).await {
            Ok(i) => vec![i],
            Err(_) => Vec::new(),
        }
    }

    fn debug_label(&self) -> &'static str {
        Self::LABEL
    }
}

impl CombobulatorWit {
    /// Continuously tick the wit at a fixed interval.
    pub async fn run(self: Arc<Self>) {
        loop {
            self.tick().await;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}

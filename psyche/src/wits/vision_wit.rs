use crate::ImageData;
use crate::ling::{Doer, Instruction};
use crate::traits::observer::SensationObserver;
use crate::traits::wit::Wit;
use crate::{Impression, Stimulus};
use async_trait::async_trait;
use lingproc::ImageData as LImageData;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{Semaphore, broadcast};
use tracing::{debug, info};

#[cfg(not(test))]
const CAPTION_COOLDOWN: Duration = Duration::from_secs(10);
#[cfg(test)]
const CAPTION_COOLDOWN: Duration = Duration::from_millis(10);
#[cfg(not(test))]
const INITIAL_DELAY: Duration = Duration::from_secs(1);
#[cfg(test)]
const INITIAL_DELAY: Duration = Duration::from_millis(0);
#[cfg(not(test))]
const RUN_INTERVAL: Duration = Duration::from_secs(1);
#[cfg(test)]
const RUN_INTERVAL: Duration = Duration::from_millis(20);

/// Wit producing first-person captions from images.
pub struct VisionWit {
    doer: Arc<dyn Doer>,
    last_caption_time: Mutex<Instant>,
    latest_image: Arc<Mutex<Option<ImageData>>>,
    llm_semaphore: Arc<Semaphore>,
    tx: Option<broadcast::Sender<crate::WitReport>>,
}

impl VisionWit {
    /// Debug label for this Wit.
    pub const LABEL: &'static str = "Vision";
    /// Create a new `VisionWit` using the provided [`Doer`].
    pub fn new(doer: Arc<dyn Doer>) -> Self {
        Self {
            doer,
            last_caption_time: Mutex::new(Instant::now() - CAPTION_COOLDOWN),
            latest_image: Arc::new(Mutex::new(None)),
            llm_semaphore: Arc::new(Semaphore::new(2)),
            tx: None,
        }
    }

    /// Create a `VisionWit` that emits [`WitReport`]s using `tx`.
    pub fn with_debug(doer: Arc<dyn Doer>, tx: broadcast::Sender<crate::WitReport>) -> Self {
        Self {
            tx: Some(tx),
            ..Self::new(doer)
        }
    }
}

#[async_trait]
impl Wit<ImageData, ImageData> for VisionWit {
    async fn observe(&self, input: ImageData) {
        *self.latest_image.lock().unwrap() = Some(input);
    }

    async fn tick(&self) -> Vec<Impression<ImageData>> {
        let now = Instant::now();
        {
            let last = self.last_caption_time.lock().unwrap();
            if now.duration_since(*last) < CAPTION_COOLDOWN {
                return Vec::new();
            }
        }

        let img = {
            let mut guard = self.latest_image.lock().unwrap();
            guard.take()
        };
        let img = match img {
            Some(i) => i,
            None => return Vec::new(),
        };
        *self.last_caption_time.lock().unwrap() = now;

        debug!("vision wit captioning image");
        let permit = self.llm_semaphore.clone().acquire_owned().await.unwrap();
        let start = Instant::now();
        let caption = if img.base64.is_empty() {
            "I can't see anything.".to_string()
        } else {
            match self
                .doer
                .follow(Instruction {
                    command: "Describe only what you see in this image in a single sentence, in the first person. Remember, this is what you are *seeing* in the first person, so unless you're looking into a mirror, you won't be seeing yourself.".into(),
                    images: vec![LImageData { mime: img.mime.clone(), base64: img.base64.clone() }],
                })
                .await
            {
                Ok(c) => c,
                Err(_) => return Vec::new(),
            }
        };
        let how = caption.trim().to_string();
        info!(elapsed=?start.elapsed(), "image captioned");
        drop(permit);
        if let Some(tx) = &self.tx {
            if crate::debug::debug_enabled(Self::LABEL).await {
                let _ = tx.send(crate::WitReport {
                    name: Self::LABEL.into(),
                    prompt: "image caption".into(),
                    output: how.clone(),
                });
            }
        }
        vec![Impression::new(
            vec![Stimulus::new(img)],
            how,
            None::<String>,
        )]
    }

    fn debug_label(&self) -> &'static str {
        Self::LABEL
    }
}

impl VisionWit {
    /// Continuously tick the wit at a fixed interval.
    pub async fn run(self: Arc<Self>) {
        tokio::time::sleep(INITIAL_DELAY).await;
        loop {
            self.tick().await;
            tokio::time::sleep(RUN_INTERVAL).await;
        }
    }

    /// Handle to the shared latest image buffer.
    pub fn latest_image_handle(&self) -> Arc<Mutex<Option<ImageData>>> {
        self.latest_image.clone()
    }
}

#[async_trait]
impl SensationObserver for VisionWit {
    async fn observe_sensation(&self, payload: &(dyn std::any::Any + Send + Sync)) {
        if let Some(sensation) = payload.downcast_ref::<crate::Sensation>() {
            if let crate::Sensation::Of(any) = sensation {
                if let Some(img) = any.downcast_ref::<ImageData>() {
                    self.observe(img.clone()).await;
                }
            }
        }
    }
}

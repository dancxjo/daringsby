use crate::ImageData;
use crate::Impression;
use crate::ling::{Doer, Instruction};
use crate::wit::Wit;
use async_trait::async_trait;
use lingproc::ImageData as LImageData;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tracing::debug;

/// Wit producing first-person captions from images.
pub struct VisionWit {
    doer: Arc<dyn Doer>,
    buffer: Mutex<Vec<ImageData>>,
    tx: Option<broadcast::Sender<crate::WitReport>>,
}

impl VisionWit {
    /// Create a new `VisionWit` using the provided [`Doer`].
    pub fn new(doer: Arc<dyn Doer>) -> Self {
        Self {
            doer,
            buffer: Mutex::new(Vec::new()),
            tx: None,
        }
    }

    /// Create a `VisionWit` that emits [`WitReport`]s using `tx`.
    pub fn with_debug(doer: Arc<dyn Doer>, tx: broadcast::Sender<crate::WitReport>) -> Self {
        Self {
            doer,
            buffer: Mutex::new(Vec::new()),
            tx: Some(tx),
        }
    }
}

#[async_trait]
impl Wit<ImageData, ImageData> for VisionWit {
    async fn observe(&self, input: ImageData) {
        self.buffer.lock().unwrap().push(input);
    }

    async fn tick(&self) -> Option<Impression<ImageData>> {
        let img = {
            let mut buf = self.buffer.lock().unwrap();
            buf.pop()
        }?;

        debug!("vision wit captioning image");
        let caption = self
            .doer
            .follow(Instruction {
                command: "You are seeing this image directly, as if with your own eyes. Describe it in a single sentence, in the first person.".into(),
                images: vec![LImageData { mime: img.mime.clone(), base64: img.base64.clone() }],
            })
            .await
            .ok()?;
        let how = caption.trim().to_string();
        if let Some(tx) = &self.tx {
            let _ = tx.send(crate::WitReport {
                name: "VisionWit".into(),
                prompt: "image caption".into(),
                output: how.clone(),
            });
        }
        Some(Impression::new(how, None::<String>, img))
    }
}

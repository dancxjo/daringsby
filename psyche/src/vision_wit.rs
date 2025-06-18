use crate::ImageData;
use crate::Impression;
use crate::ling::{Doer, Instruction};
use crate::wit::Wit;
use async_trait::async_trait;
use lingproc::ImageData as LImageData;
use std::sync::{Arc, Mutex};
use tracing::debug;

/// Wit producing first-person captions from images.
pub struct VisionWit {
    doer: Arc<dyn Doer>,
    buffer: Mutex<Vec<ImageData>>,
}

impl VisionWit {
    /// Create a new `VisionWit` using the provided [`Doer`].
    pub fn new(doer: Arc<dyn Doer>) -> Self {
        Self {
            doer,
            buffer: Mutex::new(Vec::new()),
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
        Some(Impression::new(how, None::<String>, img))
    }
}

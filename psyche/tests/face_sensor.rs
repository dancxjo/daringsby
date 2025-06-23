use async_trait::async_trait;
use futures::{StreamExt, pin_mut};
use lingproc::{Chatter, Doer, LlmInstruction, Message, TextStream, Vectorizer};
use psyche::{
    Ear, ImageData, Mouth, Psyche, Sensation, Sensor, Topic,
    sensors::face::{DummyDetector, FaceDetector, FaceInfo, FaceSensor},
    wits::memory::QdrantClient,
};
use std::sync::Arc;

#[tokio::test]
async fn emits_face_info() {
    #[derive(Clone, Default)]
    struct Dummy;
    #[async_trait]
    impl Mouth for Dummy {
        async fn speak(&self, _: &str) {}
        async fn interrupt(&self) {}
        fn speaking(&self) -> bool {
            false
        }
    }
    #[async_trait]
    impl Ear for Dummy {
        async fn hear_self_say(&self, _: &str) {}
        async fn hear_user_say(&self, _: &str) {}
    }
    #[async_trait]
    impl Doer for Dummy {
        async fn follow(&self, _: LlmInstruction) -> anyhow::Result<String> {
            Ok("ok".into())
        }
    }
    #[async_trait]
    impl Chatter for Dummy {
        async fn chat(&self, _: &str, _: &[Message]) -> anyhow::Result<lingproc::TextStream> {
            Ok(Box::pin(tokio_stream::once(Ok("hi".to_string()))))
        }
    }
    #[async_trait]
    impl Vectorizer for Dummy {
        async fn vectorize(&self, _: &str) -> anyhow::Result<Vec<f32>> {
            Ok(vec![0.0])
        }
    }
    let mouth = Arc::new(Dummy::default());
    let ear = mouth.clone();
    let psyche = Psyche::new(
        Box::new(Dummy),
        Box::new(Dummy),
        Box::new(Dummy),
        Arc::new(psyche::NoopMemory),
        mouth,
        ear,
    );
    let bus = psyche.topic_bus();
    let sensor = FaceSensor::new(
        Arc::new(DummyDetector::default()),
        QdrantClient::default(),
        bus.clone(),
    );
    let mut sub = bus.subscribe(Topic::Sensation);
    pin_mut!(sub);
    sensor
        .sense(ImageData {
            mime: "image/png".into(),
            base64: "AA==".into(),
        })
        .await;
    let sensed = sub.next().await.unwrap();
    if let Some(s) = sensed.downcast_ref::<Sensation>() {
        if let Sensation::Of(any) = s {
            let info = any.downcast_ref::<FaceInfo>().unwrap();
            assert_eq!(info.crop.mime, "image/png");
            assert_eq!(info.embedding, vec![0.0]);
        } else {
            panic!("wrong sensation")
        }
    } else {
        panic!("wrong sensation")
    }
}
struct SeqDetector {
    embeddings: std::sync::Mutex<std::vec::Vec<std::vec::Vec<f32>>>,
}

#[async_trait]
impl FaceDetector for SeqDetector {
    async fn detect_faces(&self, image: &ImageData) -> anyhow::Result<Vec<(ImageData, Vec<f32>)>> {
        let e = self.embeddings.lock().unwrap().remove(0);
        Ok(vec![(image.clone(), e)])
    }
}

#[tokio::test]
async fn skips_identical_face() {
    let bus = psyche::TopicBus::new(16);
    let detector = Arc::new(SeqDetector {
        embeddings: std::sync::Mutex::new(vec![vec![0.1, 0.0], vec![0.1, 0.0]]),
    });
    let sensor = FaceSensor::new(detector, QdrantClient::default(), bus.clone());
    let mut sub = bus.subscribe(Topic::Sensation);
    pin_mut!(sub);
    let img = ImageData {
        mime: "image/png".into(),
        base64: "AA==".into(),
    };
    sensor.sense(img.clone()).await;
    assert!(sub.next().await.is_some());
    sensor.sense(img).await;
    let second = tokio::time::timeout(std::time::Duration::from_millis(50), sub.next()).await;
    assert!(second.is_err());
}

#[tokio::test]
async fn stores_distinct_faces() {
    let bus = psyche::TopicBus::new(16);
    let detector = Arc::new(SeqDetector {
        embeddings: std::sync::Mutex::new(vec![vec![0.1, 0.0], vec![0.0, 0.1]]),
    });
    let sensor = FaceSensor::new(detector, QdrantClient::default(), bus.clone());
    let mut sub = bus.subscribe(Topic::Sensation);
    pin_mut!(sub);
    let img = ImageData {
        mime: "image/png".into(),
        base64: "AA==".into(),
    };
    sensor.sense(img.clone()).await;
    assert!(sub.next().await.is_some());
    sensor.sense(img).await;
    let second = tokio::time::timeout(std::time::Duration::from_millis(50), sub.next()).await;
    assert!(second.is_ok());
}

#[tokio::test]
async fn logs_skipped_detection() {
    use tracing_subscriber::fmt::MakeWriter;
    #[derive(Clone)]
    struct Writer(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
    impl std::io::Write for Writer {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    impl<'a> MakeWriter<'a> for Writer {
        type Writer = Writer;
        fn make_writer(&'a self) -> Self::Writer {
            Writer(self.0.clone())
        }
    }
    let buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let collector = tracing_subscriber::fmt()
        .with_writer(Writer(buf.clone()))
        .with_max_level(tracing::Level::DEBUG)
        .finish();
    let _guard = tracing::subscriber::set_default(collector);

    let bus = psyche::TopicBus::new(16);
    let detector = Arc::new(SeqDetector {
        embeddings: std::sync::Mutex::new(vec![vec![0.2, 0.0], vec![0.2, 0.0]]),
    });
    let sensor = FaceSensor::new(detector, QdrantClient::default(), bus.clone());
    sensor
        .sense(ImageData {
            mime: "image/png".into(),
            base64: "AA==".into(),
        })
        .await;
    sensor
        .sense(ImageData {
            mime: "image/png".into(),
            base64: "AA==".into(),
        })
        .await;

    drop(_guard);
    let logs = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
    assert!(logs.contains("skipping similar face detection"));
}

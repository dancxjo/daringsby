use async_trait::async_trait;
use futures::{StreamExt, pin_mut};
use psyche::ling::{Chatter, Doer, Instruction, Message, Vectorizer};
use psyche::{
    Ear, ImageData, Mouth, Psyche, Sensation, Sensor, Topic,
    sensors::face::{DummyDetector, FaceInfo, FaceSensor},
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
        async fn follow(&self, _: Instruction) -> anyhow::Result<String> {
            Ok("ok".into())
        }
    }
    #[async_trait]
    impl Chatter for Dummy {
        async fn chat(&self, _: &str, _: &[Message]) -> anyhow::Result<psyche::ling::ChatStream> {
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

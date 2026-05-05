use async_trait::async_trait;
use psyche::sensors::face::FaceInfo;
use psyche::wits::Memory;
use psyche::wits::entity_wit::{EntityWit, InMemoryEmbeddingDb};
use psyche::{ImageData, Impression, ObjectInfo, image_content_id};
use psyche::{Sensation, Wit};
use serde_json::Value;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct DummyMemory(Arc<Mutex<Vec<String>>>);

#[async_trait]
impl Memory for DummyMemory {
    async fn store(&self, imp: &Impression<Value>) -> anyhow::Result<()> {
        self.0.lock().unwrap().push(imp.summary.clone());
        Ok(())
    }
}

fn dummy_face(v: f32) -> FaceInfo {
    let crop = ImageData {
        mime: "m".into(),
        base64: "b".into(),
        captured_at: None,
    };
    FaceInfo {
        face_id: image_content_id(&crop),
        source_image_id: image_content_id(&crop),
        crop,
        embedding: vec![v],
        vector_id: None,
    }
}

fn dummy_object(v: f32) -> ObjectInfo {
    ObjectInfo {
        label: None,
        embedding: vec![v],
    }
}

#[tokio::test]
async fn deduplicates_faces() {
    let db = Arc::new(InMemoryEmbeddingDb::default());
    let wit = EntityWit::new(Arc::new(DummyMemory::default()), db.clone(), db.clone());
    wit.observe(Sensation::of(dummy_face(0.1))).await;
    let out1 = wit.tick().await;
    wit.observe(Sensation::of(dummy_face(0.1))).await;
    let out2 = wit.tick().await;
    assert!(out1[0].summary.contains("#0"));
    assert!(out2[0].summary.contains("#0"));
}

#[tokio::test]
async fn name_creates_person() {
    let db = Arc::new(InMemoryEmbeddingDb::default());
    let wit = EntityWit::new(Arc::new(DummyMemory::default()), db.clone(), db.clone());
    wit.observe(Sensation::heard_user_voice("Travis")).await;
    let out = wit.tick().await;
    assert!(out[0].summary.contains("Travis"));
    assert!(out[0].summary.contains("#0"));
}

#[tokio::test]
async fn face_and_name_link() {
    let db = Arc::new(InMemoryEmbeddingDb::default());
    let wit = EntityWit::new(Arc::new(DummyMemory::default()), db.clone(), db.clone());
    wit.observe(Sensation::of(dummy_face(0.2))).await;
    wit.observe(Sensation::heard_user_voice("Anna")).await;
    let out = wit.tick().await;
    assert!(out[0].summary.contains("Anna"));
}

#[tokio::test]
async fn dedup_objects() {
    let db = Arc::new(InMemoryEmbeddingDb::default());
    let wit = EntityWit::new(Arc::new(DummyMemory::default()), db.clone(), db.clone());
    wit.observe(Sensation::of(dummy_object(0.3))).await;
    let out1 = wit.tick().await;
    wit.observe(Sensation::of(dummy_object(0.3))).await;
    let out2 = wit.tick().await;
    assert!(out1[0].summary.contains("#0"));
    assert!(out2[0].summary.contains("#0"));
}

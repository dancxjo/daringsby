use async_trait::async_trait;
use chrono::Utc;
use psyche::{
    GeoEmbedding, GeoLoc, GraphStore, Heartbeat, ImageData, ObjectInfo, Sensation,
    SensationGraphObserver, SensationObserver, geoloc_content_id, geoloc_vector, image_content_id,
};
use serde_json::Value;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct MockGraph(Mutex<Vec<Value>>);

#[async_trait]
impl GraphStore for MockGraph {
    async fn store_data(&self, data: &Value) -> anyhow::Result<()> {
        self.0.lock().unwrap().push(data.clone());
        Ok(())
    }
}

#[tokio::test]
async fn links_geolocation_embedding_to_geolocation_node() {
    let graph = Arc::new(MockGraph::default());
    let observer = SensationGraphObserver::new(graph.clone());
    let loc = GeoLoc {
        latitude: 10.0,
        longitude: 20.0,
        observed_at: Some("2026-05-05T12:34:56Z".into()),
    };
    let geoloc_id = geoloc_content_id(&loc);
    let sensation = Sensation::of(GeoEmbedding {
        loc,
        geoloc_id: geoloc_id.clone(),
        embedding: geoloc_vector(&GeoLoc {
            latitude: 10.0,
            longitude: 20.0,
            observed_at: Some("2026-05-05T12:34:56Z".into()),
        }),
        vector_id: Some("vector-1".into()),
        model: Some("earth-unit-sphere/v1".into()),
    });

    observer.observe_sensation(&sensation).await;

    let stored = graph.0.lock().unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0]["nodes"][1]["id"], geoloc_id);
    assert_eq!(stored[0]["nodes"][2]["collection"], "geolocations");
    assert_eq!(stored[0]["nodes"][2]["database"], "qdrant");
    assert_eq!(
        stored[0]["relationships"][1]["type"],
        "HAS_GEOLOCATION_VECTOR"
    );
}

#[tokio::test]
async fn stores_original_geolocation_sensation() {
    let graph = Arc::new(MockGraph::default());
    let observer = SensationGraphObserver::new(graph.clone());
    let loc = GeoLoc {
        latitude: 10.0,
        longitude: 20.0,
        observed_at: Some("2026-05-05T12:34:56Z".into()),
    };
    let expected_id = geoloc_content_id(&loc);
    let sensation = Sensation::of(loc);

    observer.observe_sensation(&sensation).await;

    let stored = graph.0.lock().unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0]["nodes"][1]["id"], expected_id);
    assert_eq!(stored[0]["nodes"][1]["latitude"], 10.0);
    assert_eq!(stored[0]["nodes"][1]["longitude"], 20.0);
    assert_eq!(stored[0]["nodes"][1]["observed_at"], "2026-05-05T12:34:56Z");
    assert_eq!(stored[0]["relationships"][0]["type"], "OBSERVED");
}

#[tokio::test]
async fn merges_duplicate_image_sensations_once() {
    let graph = Arc::new(MockGraph::default());
    let observer = SensationGraphObserver::new(graph.clone());
    let image = ImageData {
        mime: "image/png".into(),
        base64: "zzz".into(),
        captured_at: Some("2026-05-05T12:34:56Z".into()),
    };
    let expected_id = image_content_id(&image);
    let sensation = Sensation::of(image.clone());

    observer.observe_sensation(&sensation).await;
    observer.observe_sensation(&sensation).await;

    let stored = graph.0.lock().unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0]["nodes"][1]["id"], expected_id);
    assert_eq!(stored[0]["nodes"][1]["base64"], "zzz");
    assert_eq!(stored[0]["nodes"][1]["captured_at"], "2026-05-05T12:34:56Z");
    assert_eq!(stored[0]["relationships"][0]["type"], "OBSERVED");
}

#[tokio::test]
async fn stores_heartbeat_sensation() {
    let graph = Arc::new(MockGraph::default());
    let observer = SensationGraphObserver::new(graph.clone());
    let timestamp = chrono::DateTime::parse_from_rfc3339("2026-05-05T12:34:56Z")
        .unwrap()
        .with_timezone(&Utc);
    let sensation = Sensation::of_at(Heartbeat { timestamp }, timestamp);

    observer.observe_sensation(&sensation).await;

    let stored = graph.0.lock().unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0]["nodes"][1]["label"], "Heartbeat");
    assert_eq!(
        stored[0]["nodes"][1]["timestamp"],
        "2026-05-05T12:34:56+00:00"
    );
    assert_eq!(stored[0]["relationships"][0]["type"], "OBSERVED");
}

#[tokio::test]
async fn stores_object_sensation_without_embedding_payload() {
    let graph = Arc::new(MockGraph::default());
    let observer = SensationGraphObserver::new(graph.clone());
    let sensation = Sensation::of(ObjectInfo {
        label: Some("mug".into()),
        embedding: vec![0.1, 0.2, 0.3],
    });

    observer.observe_sensation(&sensation).await;

    let stored = graph.0.lock().unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0]["nodes"][1]["label"], "ObjectObservation");
    assert_eq!(stored[0]["nodes"][1]["object_label"], "mug");
    assert_eq!(stored[0]["nodes"][1]["embedding_len"], 3);
    assert!(stored[0]["nodes"][1].get("embedding").is_none());
}

#[tokio::test]
async fn stores_unknown_sensation_marker() {
    let graph = Arc::new(MockGraph::default());
    let observer = SensationGraphObserver::new(graph.clone());
    let sensation = Sensation::of(42_u64);

    observer.observe_sensation(&sensation).await;

    let stored = graph.0.lock().unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0]["nodes"][1]["label"], "UnknownSensation");
    assert_eq!(stored[0]["relationships"][0]["type"], "OBSERVED");
}

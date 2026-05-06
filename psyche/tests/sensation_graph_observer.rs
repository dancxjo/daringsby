use async_trait::async_trait;
use chrono::Utc;
use psyche::{
    AudioClip, BrowserMotion, CombobulationSummary, DeviceOrientation, GeoEmbedding, GeoLoc,
    GraphStore, Heartbeat, ImageData, MotionVector, ObjectInfo, Sensation, SensationGraphObserver,
    SensationObserver, audio_clip_id, geoloc_content_id, geoloc_vector, image_content_id,
};
use serde_json::{Value, json};
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
async fn stores_browser_motion_sensation() {
    let graph = Arc::new(MockGraph::default());
    let observer = SensationGraphObserver::new(graph.clone());
    let motion = BrowserMotion {
        acceleration: Some(MotionVector {
            x: Some(1.0),
            y: Some(2.0),
            z: Some(3.0),
        }),
        acceleration_including_gravity: Some(MotionVector {
            x: Some(1.1),
            y: Some(2.1),
            z: Some(12.8),
        }),
        rotation_rate: Some(DeviceOrientation {
            alpha: Some(4.0),
            beta: Some(5.0),
            gamma: Some(6.0),
            absolute: None,
        }),
        orientation: Some(DeviceOrientation {
            alpha: Some(7.0),
            beta: Some(8.0),
            gamma: Some(9.0),
            absolute: Some(true),
        }),
        interval: Some(16.7),
        observed_at: Some("2026-05-06T12:00:00Z".into()),
    };

    observer.observe_sensation(&Sensation::of(motion)).await;

    let stored = graph.0.lock().unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0]["nodes"][1]["label"], "BrowserMotion");
    assert_eq!(stored[0]["nodes"][1]["acceleration"]["x"], 1.0);
    assert_eq!(
        stored[0]["nodes"][1]["acceleration_including_gravity"]["z"],
        12.8
    );
    assert_eq!(stored[0]["nodes"][1]["rotation_rate"]["gamma"], 6.0);
    assert_eq!(stored[0]["nodes"][1]["orientation"]["absolute"], true);
    assert_eq!(stored[0]["nodes"][1]["interval"], 16.7);
    assert_eq!(stored[0]["relationships"][0]["type"], "OBSERVED");
}

#[tokio::test]
async fn stores_audio_sensation_with_transcript() {
    let graph = Arc::new(MockGraph::default());
    let observer = SensationGraphObserver::new(graph.clone());
    let audio = AudioClip {
        mime: "audio/wav".into(),
        base64: "UklGRg==".into(),
        sample_rate: 16_000,
        channels: 1,
        transcript: Some("hello there".into()),
        captured_at: Some("2026-05-05T12:34:56Z".into()),
    };
    let expected_id = audio_clip_id(&audio);

    observer.observe_sensation(&Sensation::of(audio)).await;

    let stored = graph.0.lock().unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0]["nodes"][1]["label"], "AudioClip");
    assert_eq!(stored[0]["nodes"][1]["id"], expected_id);
    assert_eq!(stored[0]["nodes"][1]["transcript"], "hello there");
    assert_eq!(stored[0]["relationships"][0]["type"], "OBSERVED");
}

#[tokio::test]
async fn stores_combobulation_summary_as_sensation() {
    let graph = Arc::new(MockGraph::default());
    let observer = SensationGraphObserver::new(graph.clone());
    let occurred_at = Utc::now();

    observer
        .observe_sensation(&Sensation::of_at(
            CombobulationSummary {
                text: "I may be hearing someone nearby.".into(),
                created_at: Some(occurred_at.to_rfc3339()),
                source_sensation_ids: vec!["sensation:audio:1".into()],
            },
            occurred_at,
        ))
        .await;

    let stored = graph.0.lock().unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0]["nodes"][0]["kind"], "combobulation_summary");
    assert_eq!(stored[0]["nodes"][1]["label"], "CombobulationSummary");
    assert_eq!(
        stored[0]["nodes"][1]["text"],
        "I may be hearing someone nearby."
    );
    assert_eq!(stored[0]["relationships"][0]["type"], "OBSERVED");
    assert_eq!(stored[0]["relationships"][1]["type"], "DERIVED_FROM");
    assert_eq!(stored[0]["relationships"][1]["to"], "sensation:audio:1");
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
async fn stores_json_sensation_payload() {
    let graph = Arc::new(MockGraph::default());
    let observer = SensationGraphObserver::new(graph.clone());
    let payload = json!({
        "kind": "browser-event",
        "value": 7,
    });
    let sensation = Sensation::of(payload.clone());

    observer.observe_sensation(&sensation).await;

    let stored = graph.0.lock().unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0]["nodes"][1]["label"], "JsonSensation");
    assert_eq!(stored[0]["nodes"][1]["value"], payload);
    assert_eq!(stored[0]["relationships"][0]["type"], "OBSERVED");
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

use async_trait::async_trait;
use psyche::{
    GeoEmbedding, GeoLoc, GraphStore, ImageData, Sensation, SensationGraphObserver,
    SensationObserver, geoloc_content_id, geoloc_vector, image_content_id,
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
    assert_eq!(stored[0]["nodes"][0]["id"], geoloc_id);
    assert_eq!(stored[0]["nodes"][1]["collection"], "geolocations");
    assert_eq!(
        stored[0]["relationships"][0]["type"],
        "HAS_GEOLOCATION_VECTOR"
    );
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
    assert_eq!(stored[0]["nodes"][0]["id"], expected_id);
    assert_eq!(stored[0]["nodes"][0]["captured_at"], "2026-05-05T12:34:56Z");
}

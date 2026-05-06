use httpmock::{Method::DELETE, Method::GET, Method::POST, Method::PUT, MockServer};
use psyche::{QdrantClient, qdrant_vector_collections};

#[test]
fn qdrant_vector_collections_lists_all_written_vector_collections() {
    assert_eq!(
        qdrant_vector_collections(),
        &[
            "memories",
            "images",
            "image_descriptions",
            "scene_vectors",
            "faces",
            "geolocations",
            "voices",
        ]
    );
}

#[tokio::test]
async fn store_face_vector_creates_collection_and_upserts_point() {
    let server = MockServer::start_async().await;
    let get_collection = server
        .mock_async(|when, then| {
            when.method(GET).path("/collections/faces");
            then.status(404).body("{}");
        })
        .await;
    let create_collection = server
        .mock_async(|when, then| {
            when.method(PUT)
                .path("/collections/faces")
                .body_contains("\"size\":2")
                .body_contains("\"distance\":\"Cosine\"");
            then.status(200).body(r#"{"result":true,"status":"ok"}"#);
        })
        .await;
    let upsert_point = server
        .mock_async(|when, then| {
            when.method(PUT)
                .path("/collections/faces/points")
                .query_param("wait", "true")
                .body_contains("\"kind\":\"face\"")
                .body_contains("\"face_id\":\"face:1\"")
                .body_contains("\"neo4j_node_id\":\"face:1\"")
                .body_contains("\"source_image_id\":\"image:1\"")
                .body_contains("\"vector\"")
                .body_contains("1.0")
                .body_contains("2.0");
            then.status(200)
                .body(r#"{"result":{"operation_id":1},"status":"ok"}"#);
        })
        .await;

    QdrantClient::new(server.base_url())
        .store_face_vector_for(Some("face:1"), Some("image:1"), &[1.0, 2.0])
        .await
        .unwrap();

    get_collection.assert_async().await;
    create_collection.assert_async().await;
    upsert_point.assert_async().await;
}

#[tokio::test]
async fn store_face_vector_can_reference_source_sensation() {
    let server = MockServer::start_async().await;
    let get_collection = server
        .mock_async(|when, then| {
            when.method(GET).path("/collections/faces");
            then.status(200).body(
                r#"{"result":{"config":{"params":{"vectors":{"size":2,"distance":"Cosine"}}}},"status":"ok"}"#,
            );
        })
        .await;
    let upsert_point = server
        .mock_async(|when, then| {
            when.method(PUT)
                .path("/collections/faces/points")
                .query_param("wait", "true")
                .body_contains("\"face_id\":\"face:1\"")
                .body_contains("\"source_image_id\":\"image:1\"")
                .body_contains("\"sensation_id\":\"sensation:image:1\"");
            then.status(200)
                .body(r#"{"result":{"operation_id":1},"status":"ok"}"#);
        })
        .await;

    QdrantClient::new(server.base_url())
        .store_face_vector_for_sensation(
            Some("face:1"),
            Some("image:1"),
            Some("sensation:image:1"),
            &[1.0, 2.0],
        )
        .await
        .unwrap();

    get_collection.assert_async().await;
    upsert_point.assert_async().await;
}

#[tokio::test]
async fn store_geolocation_vector_creates_collection_and_upserts_point() {
    let server = MockServer::start_async().await;
    let get_collection = server
        .mock_async(|when, then| {
            when.method(GET).path("/collections/geolocations");
            then.status(404).body("{}");
        })
        .await;
    let create_collection = server
        .mock_async(|when, then| {
            when.method(PUT)
                .path("/collections/geolocations")
                .body_contains("\"size\":3")
                .body_contains("\"distance\":\"Cosine\"");
            then.status(200).body(r#"{"result":true,"status":"ok"}"#);
        })
        .await;
    let upsert_point = server
        .mock_async(|when, then| {
            when.method(PUT)
                .path("/collections/geolocations/points")
                .query_param("wait", "true")
                .body_contains("\"kind\":\"geolocation\"")
                .body_contains("\"geoloc_id\":\"geolocation:1\"")
                .body_contains("\"neo4j_node_id\":\"geolocation:1\"")
                .body_contains("\"latitude\":10.0")
                .body_contains("\"longitude\":20.0");
            then.status(200)
                .body(r#"{"result":{"operation_id":1},"status":"ok"}"#);
        })
        .await;

    QdrantClient::new(server.base_url())
        .store_geolocation_vector_for("geolocation:1", 10.0, 20.0, &[1.0, 0.0, 0.0])
        .await
        .unwrap();

    get_collection.assert_async().await;
    create_collection.assert_async().await;
    upsert_point.assert_async().await;
}

#[tokio::test]
async fn store_image_vector_creates_collection_and_upserts_point() {
    let server = MockServer::start_async().await;
    let get_collection = server
        .mock_async(|when, then| {
            when.method(GET).path("/collections/images");
            then.status(404).body("{}");
        })
        .await;
    let create_collection = server
        .mock_async(|when, then| {
            when.method(PUT)
                .path("/collections/images")
                .body_contains("\"size\":2")
                .body_contains("\"distance\":\"Cosine\"");
            then.status(200).body(r#"{"result":true,"status":"ok"}"#);
        })
        .await;
    let upsert_point = server
        .mock_async(|when, then| {
            when.method(PUT)
                .path("/collections/images/points")
                .query_param("wait", "true")
                .body_contains("\"kind\":\"image\"")
                .body_contains("\"image_id\":\"image:1\"")
                .body_contains("\"neo4j_node_id\":\"image:1\"")
                .body_contains("\"vector\"");
            then.status(200)
                .body(r#"{"result":{"operation_id":1},"status":"ok"}"#);
        })
        .await;

    QdrantClient::new(server.base_url())
        .store_image_vector("image:1", &[1.0, 2.0])
        .await
        .unwrap();

    get_collection.assert_async().await;
    create_collection.assert_async().await;
    upsert_point.assert_async().await;
}

#[tokio::test]
async fn store_image_description_vector_uses_own_collection() {
    let server = MockServer::start_async().await;
    let get_collection = server
        .mock_async(|when, then| {
            when.method(GET).path("/collections/image_descriptions");
            then.status(404).body("{}");
        })
        .await;
    let create_collection = server
        .mock_async(|when, then| {
            when.method(PUT)
                .path("/collections/image_descriptions")
                .body_contains("\"size\":2");
            then.status(200).body(r#"{"result":true,"status":"ok"}"#);
        })
        .await;
    let upsert_point = server
        .mock_async(|when, then| {
            when.method(PUT)
                .path("/collections/image_descriptions/points")
                .query_param("wait", "true")
                .body_contains("\"kind\":\"image_description\"")
                .body_contains("\"image_id\":\"image:1\"")
                .body_contains("\"neo4j_node_id\":\"image:1\"")
                .body_contains("\"description\":\"I see a test.\"");
            then.status(200)
                .body(r#"{"result":{"operation_id":1},"status":"ok"}"#);
        })
        .await;

    QdrantClient::new(server.base_url())
        .store_image_description_vector("image:1", "I see a test.", &[1.0, 2.0])
        .await
        .unwrap();

    get_collection.assert_async().await;
    create_collection.assert_async().await;
    upsert_point.assert_async().await;
}

#[tokio::test]
async fn store_scene_vector_uses_own_collection_and_links_source_image() {
    let server = MockServer::start_async().await;
    let get_collection = server
        .mock_async(|when, then| {
            when.method(GET).path("/collections/scene_vectors");
            then.status(404).body("{}");
        })
        .await;
    let create_collection = server
        .mock_async(|when, then| {
            when.method(PUT)
                .path("/collections/scene_vectors")
                .body_contains("\"size\":2");
            then.status(200).body(r#"{"result":true,"status":"ok"}"#);
        })
        .await;
    let upsert_point = server
        .mock_async(|when, then| {
            when.method(PUT)
                .path("/collections/scene_vectors/points")
                .query_param("wait", "true")
                .body_contains("\"kind\":\"scene\"")
                .body_contains("\"image_id\":\"image:1\"")
                .body_contains("\"neo4j_node_id\":\"image:1\"")
                .body_contains("\"source_image_id\":\"image:1\"")
                .body_contains("\"sensation_id\":\"sensation:image:1\"")
                .body_contains("\"model\":\"clip-test\"");
            then.status(200)
                .body(r#"{"result":{"operation_id":1},"status":"ok"}"#);
        })
        .await;

    QdrantClient::new(server.base_url())
        .store_scene_vector_for_sensation(
            "image:1",
            Some("sensation:image:1"),
            "clip-test",
            &[1.0, 2.0],
        )
        .await
        .unwrap();

    get_collection.assert_async().await;
    create_collection.assert_async().await;
    upsert_point.assert_async().await;
}

#[tokio::test]
async fn store_vector_uses_existing_memory_collection() {
    let server = MockServer::start_async().await;
    let get_collection = server
        .mock_async(|when, then| {
            when.method(GET).path("/collections/memories");
            then.status(200).body(
                r#"{"result":{"config":{"params":{"vectors":{"size":1,"distance":"Cosine"}}}},"status":"ok"}"#,
            );
        })
        .await;
    let upsert_point = server
        .mock_async(|when, then| {
            when.method(PUT)
                .path("/collections/memories/points")
                .query_param("wait", "true")
                .body_contains("\"kind\":\"memory\"")
                .body_contains("\"headline\":\"hello\"")
                .body_contains("\"neo4j_node_id\":\"impression:1\"")
                .body_contains("\"vector\"");
            then.status(200)
                .body(r#"{"result":{"operation_id":1},"status":"ok"}"#);
        })
        .await;

    QdrantClient::new(server.base_url())
        .store_vector_for_node("hello", Some("impression:1"), &[3.0])
        .await
        .unwrap();

    get_collection.assert_async().await;
    upsert_point.assert_async().await;
}

#[tokio::test]
async fn store_voice_vector_creates_collection_and_upserts_point() {
    let server = MockServer::start_async().await;
    let get_collection = server
        .mock_async(|when, then| {
            when.method(GET).path("/collections/voices");
            then.status(404).body("{}");
        })
        .await;
    let create_collection = server
        .mock_async(|when, then| {
            when.method(PUT)
                .path("/collections/voices")
                .body_contains("\"size\":2")
                .body_contains("\"distance\":\"Cosine\"");
            then.status(200).body(r#"{"result":true,"status":"ok"}"#);
        })
        .await;
    let upsert_point = server
        .mock_async(|when, then| {
            when.method(PUT)
                .path("/collections/voices/points")
                .query_param("wait", "true")
                .body_contains("\"kind\":\"voice\"")
                .body_contains("\"clip_id\":\"audio:1\"")
                .body_contains("\"neo4j_node_id\":\"audio:1\"")
                .body_contains("\"vector\"")
                .body_contains("1.0")
                .body_contains("2.0");
            then.status(200)
                .body(r#"{"result":{"operation_id":1},"status":"ok"}"#);
        })
        .await;

    QdrantClient::new(server.base_url())
        .store_voice_vector_for(Some("audio:1"), &[1.0, 2.0])
        .await
        .unwrap();

    get_collection.assert_async().await;
    create_collection.assert_async().await;
    upsert_point.assert_async().await;
}

#[tokio::test]
async fn store_voice_vector_can_reference_source_sensation_and_user() {
    let server = MockServer::start_async().await;
    let get_collection = server
        .mock_async(|when, then| {
            when.method(GET).path("/collections/voices");
            then.status(200).body(
                r#"{"result":{"config":{"params":{"vectors":{"size":2,"distance":"Cosine"}}}},"status":"ok"}"#,
            );
        })
        .await;
    let upsert_point = server
        .mock_async(|when, then| {
            when.method(PUT)
                .path("/collections/voices/points")
                .query_param("wait", "true")
                .body_contains("\"kind\":\"voice\"")
                .body_contains("\"clip_id\":\"audio:1\"")
                .body_contains("\"sensation_id\":\"sensation:audio:1\"")
                .body_contains("\"user_id\":\"speaker:1\"");
            then.status(200)
                .body(r#"{"result":{"operation_id":1},"status":"ok"}"#);
        })
        .await;

    QdrantClient::new(server.base_url())
        .store_voice_vector_for_sensation(
            Some("audio:1"),
            Some("sensation:audio:1"),
            Some("speaker:1"),
            &[1.0, 2.0],
        )
        .await
        .unwrap();

    get_collection.assert_async().await;
    upsert_point.assert_async().await;
}

#[tokio::test]
async fn empty_vectors_are_rejected_before_network_request() {
    let server = MockServer::start_async().await;

    let err = QdrantClient::new(server.base_url())
        .store_face_vector(&[])
        .await
        .unwrap_err();

    assert!(err.to_string().contains("empty vector"));
}

#[tokio::test]
async fn existing_collection_dimension_mismatch_recreates_collection_before_upsert() {
    let server = MockServer::start_async().await;
    let get_collection = server
        .mock_async(|when, then| {
            when.method(GET).path("/collections/faces");
            then.status(200).body(
                r#"{"result":{"config":{"params":{"vectors":{"size":2,"distance":"Cosine"}}}},"status":"ok"}"#,
            );
        })
        .await;
    let delete_collection = server
        .mock_async(|when, then| {
            when.method(DELETE).path("/collections/faces");
            then.status(200).body(r#"{"result":true,"status":"ok"}"#);
        })
        .await;
    let create_collection = server
        .mock_async(|when, then| {
            when.method(PUT)
                .path("/collections/faces")
                .body_contains("\"size\":512")
                .body_contains("\"distance\":\"Cosine\"");
            then.status(200).body(r#"{"result":true,"status":"ok"}"#);
        })
        .await;
    let upsert_point = server
        .mock_async(|when, then| {
            when.method(PUT)
                .path("/collections/faces/points")
                .query_param("wait", "true")
                .body_contains("\"vector\"");
            then.status(200)
                .body(r#"{"result":{"operation_id":1},"status":"ok"}"#);
        })
        .await;

    QdrantClient::new(server.base_url())
        .store_face_vector(&vec![0.0; 512])
        .await
        .unwrap();

    get_collection.assert_async().await;
    delete_collection.assert_async().await;
    create_collection.assert_async().await;
    upsert_point.assert_async().await;
}

#[tokio::test]
async fn scroll_vectors_reads_points_with_payloads_and_vectors() {
    let server = MockServer::start_async().await;
    let scroll = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/collections/memories/points/scroll")
                .body_contains("\"with_payload\":true")
                .body_contains("\"with_vector\":true")
                .body_contains("\"limit\":2");
            then.status(200).json_body(serde_json::json!({
                "result": {
                    "points": [
                        {
                            "id": "point-1",
                            "payload": {"neo4j_node_id": "memory:1"},
                            "vector": [1.0, 0.0]
                        },
                        {
                            "id": "point-2",
                            "payload": {"neo4j_node_id": "memory:2"},
                            "vector": [0.9, 0.1]
                        }
                    ],
                    "next_page_offset": null
                },
                "status": "ok"
            }));
        })
        .await;

    let points = QdrantClient::new(server.base_url())
        .scroll_vectors("memories", 10, 2)
        .await
        .unwrap();

    assert_eq!(points.len(), 2);
    assert_eq!(points[0].point_id, "point-1");
    assert_eq!(points[0].vector, vec![1.0, 0.0]);
    assert_eq!(points[0].payload["neo4j_node_id"], "memory:1");
    scroll.assert_async().await;
}

#[tokio::test]
async fn scroll_vectors_if_collection_exists_returns_none_for_missing_collection() {
    let server = MockServer::start_async().await;
    let scroll = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/collections/voices/points/scroll")
                .body_contains("\"with_payload\":true")
                .body_contains("\"with_vector\":true");
            then.status(404).body("{}");
        })
        .await;

    let points = QdrantClient::new(server.base_url())
        .scroll_vectors_if_collection_exists("voices", 10, 2)
        .await
        .unwrap();

    assert_eq!(points, None);
    scroll.assert_async().await;
}

#[tokio::test]
async fn collection_exists_reports_present_and_missing_collections() {
    let server = MockServer::start_async().await;
    let present = server
        .mock_async(|when, then| {
            when.method(GET).path("/collections/memories");
            then.status(200).body("{}");
        })
        .await;
    let missing = server
        .mock_async(|when, then| {
            when.method(GET).path("/collections/voices");
            then.status(404).body("{}");
        })
        .await;

    let qdrant = QdrantClient::new(server.base_url());

    assert!(qdrant.collection_exists("memories").await.unwrap());
    assert!(!qdrant.collection_exists("voices").await.unwrap());
    present.assert_async().await;
    missing.assert_async().await;
}

#[tokio::test]
async fn delete_collection_if_exists_deletes_present_and_ignores_missing_collection() {
    let server = MockServer::start_async().await;
    let present = server
        .mock_async(|when, then| {
            when.method(DELETE).path("/collections/memories");
            then.status(200).body("{}");
        })
        .await;
    let missing = server
        .mock_async(|when, then| {
            when.method(DELETE).path("/collections/voices");
            then.status(404).body("{}");
        })
        .await;

    let qdrant = QdrantClient::new(server.base_url());

    assert!(
        qdrant
            .delete_collection_if_exists("memories")
            .await
            .unwrap()
    );
    assert!(!qdrant.delete_collection_if_exists("voices").await.unwrap());
    present.assert_async().await;
    missing.assert_async().await;
}

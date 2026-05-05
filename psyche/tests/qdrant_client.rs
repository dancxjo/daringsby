use httpmock::{Method::DELETE, Method::GET, Method::PUT, MockServer};
use psyche::QdrantClient;

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
                .body_contains("\"vector\"")
                .body_contains("1.0")
                .body_contains("2.0");
            then.status(200)
                .body(r#"{"result":{"operation_id":1},"status":"ok"}"#);
        })
        .await;

    QdrantClient::new(server.base_url())
        .store_face_vector(&[1.0, 2.0])
        .await
        .unwrap();

    get_collection.assert_async().await;
    create_collection.assert_async().await;
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
                .body_contains("\"vector\"");
            then.status(200)
                .body(r#"{"result":{"operation_id":1},"status":"ok"}"#);
        })
        .await;

    QdrantClient::new(server.base_url())
        .store_vector("hello", &[3.0])
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
                .body_contains("\"vector\"")
                .body_contains("1.0")
                .body_contains("2.0");
            then.status(200)
                .body(r#"{"result":{"operation_id":1},"status":"ok"}"#);
        })
        .await;

    QdrantClient::new(server.base_url())
        .store_voice_vector(&[1.0, 2.0])
        .await
        .unwrap();

    get_collection.assert_async().await;
    create_collection.assert_async().await;
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

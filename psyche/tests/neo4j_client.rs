use httpmock::{Method::POST, MockServer};
use psyche::Neo4jClient;
use serde_json::json;

#[tokio::test]
async fn neo4j_client_converts_bolt_uri_to_http_commit_endpoint() {
    let server = MockServer::start_async().await;
    let host = server.address().ip();
    let http_port = server.address().port();
    let bolt_port = if http_port == 7687 { 7688 } else { http_port };
    let commit = server
        .mock_async(|when, then| {
            when.method(POST).path("/db/neo4j/tx/commit");
            then.status(200).body(r#"{"results":[{}],"errors":[]}"#);
        })
        .await;

    Neo4jClient::new(
        format!("bolt://{host}:{bolt_port}"),
        "neo4j".into(),
        "password".into(),
    )
    .store_data(&json!({
        "op": "merge_graph",
        "nodes": [{
            "label": "Image",
            "id": "image:1",
        }],
        "relationships": [],
    }))
    .await
    .unwrap();

    commit.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_commits_merge_graph_records() {
    let server = MockServer::start_async().await;
    let commit = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("CREATE CONSTRAINT pete_graph_node_id")
                .body_contains("MERGE (n:GraphNode")
                .body_contains("HAS_IMAGE_VECTOR")
                .body_contains("qdrant:images:point-1");
            then.status(200).body(r#"{"results":[{}],"errors":[]}"#);
        })
        .await;

    Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .store_data(&json!({
            "op": "merge_graph",
            "nodes": [
                {
                    "label": "Image",
                    "id": "image:1",
                    "mime": "image/jpeg",
                },
                {
                    "label": "Vector",
                    "id": "qdrant:images:point-1",
                    "database": "qdrant",
                    "collection": "images",
                    "point_id": "point-1",
                    "kind": "image",
                }
            ],
            "relationships": [{
                "from": "image:1",
                "to": "qdrant:images:point-1",
                "type": "HAS_IMAGE_VECTOR",
            }],
        }))
        .await
        .unwrap();

    commit.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_reports_transaction_errors() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(POST).path("/db/neo4j/tx/commit");
            then.status(200)
                .body(r#"{"results":[],"errors":[{"code":"Neo.ClientError","message":"bad"}]}"#);
        })
        .await;

    let err = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .store_data(&json!({
            "op": "merge_graph",
            "nodes": [{
                "label": "Image",
                "id": "image:1",
            }],
            "relationships": [],
        }))
        .await
        .unwrap_err();

    assert!(err.to_string().contains("Neo4j returned errors"));
}

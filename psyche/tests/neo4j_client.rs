use httpmock::{Method::POST, MockServer};
use psyche::{GraphSpeechSegment, Neo4jClient};
use serde_json::json;

#[tokio::test]
async fn neo4j_client_converts_bolt_uri_to_http_commit_endpoint() {
    let server = MockServer::start_async().await;
    let host = server.address().ip();
    let http_port = server.address().port();
    let bolt_port = if http_port == 7687 { 7688 } else { http_port };
    let constraint = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("CREATE CONSTRAINT pete_graph_node_id");
            then.status(200).body(r#"{"results":[{}],"errors":[]}"#);
        })
        .await;
    let commit = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MERGE (n:GraphNode");
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

    constraint.assert_async().await;
    commit.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_commits_merge_graph_records() {
    let server = MockServer::start_async().await;
    let constraint = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("CREATE CONSTRAINT pete_graph_node_id");
            then.status(200).body(r#"{"results":[{}],"errors":[]}"#);
        })
        .await;
    let commit = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
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

    constraint.assert_async().await;
    commit.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_reports_transaction_errors() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("CREATE CONSTRAINT pete_graph_node_id");
            then.status(200).body(r#"{"results":[{}],"errors":[]}"#);
        })
        .await;
    server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MERGE (n:GraphNode");
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

#[tokio::test]
async fn neo4j_client_ensures_constraint_once_per_client() {
    let server = MockServer::start_async().await;
    let constraint = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("CREATE CONSTRAINT pete_graph_node_id");
            then.status(200).body(r#"{"results":[{}],"errors":[]}"#);
        })
        .await;
    let commit = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MERGE (n:GraphNode");
            then.status(200).body(r#"{"results":[{}],"errors":[]}"#);
        })
        .await;
    let client = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into());
    let record = json!({
        "op": "merge_graph",
        "nodes": [{
            "label": "Image",
            "id": "image:1",
        }],
        "relationships": [],
    });

    client.store_data(&record).await.unwrap();
    client.store_data(&record).await.unwrap();

    constraint.assert_hits_async(1).await;
    commit.assert_hits_async(2).await;
}

#[tokio::test]
async fn neo4j_client_loads_latest_untranscribed_audio_clip() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MATCH (a:GraphNode:AudioClip)")
                .body_contains("a.transcript IS NULL")
                .body_contains("NOT (a)-[:HAS_TRANSCRIPTION]->(:GraphNode:Transcription)")
                .body_contains("ORDER BY observed_at DESC");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": [
                        "a.id",
                        "a.mime",
                        "a.base64",
                        "a.sample_rate",
                        "a.channels",
                        "a.captured_at",
                        "a.occurred_at"
                    ],
                    "data": [{
                        "row": [
                            "audio:1",
                            "audio/pcm;format=s16le;rate=16000",
                            "AAA=",
                            16000,
                            1,
                            "2026-05-05T12:34:56Z",
                            "2026-05-05T12:34:57Z"
                        ]
                    }]
                }],
                "errors": []
            }));
        })
        .await;

    let clip = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .latest_untranscribed_audio_clip()
        .await
        .unwrap()
        .unwrap();

    assert_eq!(clip.id, "audio:1");
    assert_eq!(clip.clip.mime, "audio/pcm;format=s16le;rate=16000");
    assert_eq!(clip.clip.base64, "AAA=");
    assert_eq!(clip.clip.sample_rate, 16000);
    assert_eq!(clip.clip.channels, 1);
    assert_eq!(
        clip.clip.captured_at.as_deref(),
        Some("2026-05-05T12:34:56Z")
    );
    assert_eq!(clip.occurred_at.as_deref(), Some("2026-05-05T12:34:57Z"));
    query.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_loads_graph_snapshot() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MATCH (n:GraphNode)")
                .body_contains("LIMIT $limit")
                .body_contains("\"limit\":25");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": ["nodes", "relationships"],
                    "data": [{
                        "row": [
                            [{
                                "id": "image:1",
                                "labels": ["GraphNode", "Image"],
                                "properties": {
                                    "id": "image:1",
                                    "mime": "image/jpeg",
                                    "base64": "too-large"
                                }
                            }, {
                                "id": "qdrant:images:point-1",
                                "labels": ["GraphNode", "Vector"],
                                "properties": {
                                    "id": "qdrant:images:point-1",
                                    "collection": "images"
                                }
                            }],
                            [{
                                "id": "5:abc:9",
                                "source": "image:1",
                                "target": "qdrant:images:point-1",
                                "type": "HAS_IMAGE_VECTOR",
                                "properties": {
                                    "type": "HAS_IMAGE_VECTOR"
                                }
                            }]
                        ]
                    }]
                }],
                "errors": []
            }));
        })
        .await;

    let snapshot = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .graph_snapshot(25)
        .await
        .unwrap();

    assert_eq!(snapshot.nodes.len(), 2);
    assert_eq!(snapshot.nodes[0].id, "image:1");
    assert_eq!(snapshot.nodes[0].labels, vec!["GraphNode", "Image"]);
    assert_eq!(snapshot.nodes[0].properties["mime"], "image/jpeg");
    assert!(snapshot.nodes[0].properties.get("base64").is_none());
    assert_eq!(snapshot.relationships.len(), 1);
    assert_eq!(snapshot.relationships[0].source, "image:1");
    assert_eq!(snapshot.relationships[0].target, "qdrant:images:point-1");
    assert_eq!(
        snapshot.relationships[0].relationship_type,
        "HAS_IMAGE_VECTOR"
    );
    query.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_attaches_audio_transcription() {
    let server = MockServer::start_async().await;
    let constraint = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("CREATE CONSTRAINT pete_graph_node_id");
            then.status(200).body(r#"{"results":[{}],"errors":[]}"#);
        })
        .await;
    let update = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MATCH (a:GraphNode:AudioClip {id: $id})")
                .body_contains("\"id\":\"audio:1\"")
                .body_contains("\"transcript\":\"hello there\"")
                .body_contains("transcribed_at")
                .body_contains("Transcription")
                .body_contains("SpeechSegment")
                .body_contains("HAS_TRANSCRIPTION")
                .body_contains("HAS_SEGMENT")
                .body_contains("SEGMENT_OF")
                .body_contains("\"start_ms\":250")
                .body_contains("\"end_ms\":1250")
                .body_contains("2026-05-05T12:34:56.250+00:00");
            then.status(200).body(r#"{"results":[{}],"errors":[]}"#);
        })
        .await;

    Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .attach_audio_transcription(
            "audio:1",
            "hello there",
            Some("2026-05-05T12:34:56Z"),
            &[GraphSpeechSegment {
                index: 0,
                text: "hello there".into(),
                start_ms: 250,
                end_ms: 1250,
                occurred_at: Some("2026-05-05T12:34:56.250+00:00".into()),
                ended_at: Some("2026-05-05T12:34:57.250+00:00".into()),
            }],
        )
        .await
        .unwrap();

    constraint.assert_async().await;
    update.assert_async().await;
}

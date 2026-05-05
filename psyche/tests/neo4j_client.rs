use chrono::Utc;
use httpmock::{Method::POST, MockServer};
use psyche::{
    GeoLoc, GraphAudioSourceSpan, GraphAwareness, GraphFaceDetection, GraphGeolocation,
    GraphImageDescription, GraphImageFrame, GraphSceneVectorization, GraphSpeechSegment,
    GraphTimelineItem, GraphTimelineWindow, GraphVoiceClip, GraphVoiceRecognition,
    GraphVoiceSample, GraphVoiceSignature, ImageData, Neo4jClient, VectorCluster,
    VectorClusterMember,
};
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
                .body_contains("OPTIONAL MATCH (s:GraphNode:Sensation)-[:OBSERVED]->(a)")
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
                        "a.occurred_at",
                        "s.id"
                    ],
                    "data": [{
                        "row": [
                            "audio:1",
                            "audio/pcm;format=s16le;rate=16000",
                            "AAA=",
                            16000,
                            1,
                            "2026-05-05T12:34:56Z",
                            "2026-05-05T12:34:57Z",
                            "sensation:audio:1"
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
    assert_eq!(clip.sensation_id.as_deref(), Some("sensation:audio:1"));
    query.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_loads_latest_audio_clip_window_for_big_transcription() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MATCH (anchor:GraphNode:AudioClip)")
                .body_contains("HAS_BIG_TRANSCRIPTION")
                .body_contains("OPTIONAL MATCH (s:GraphNode:Sensation)-[:OBSERVED]->(a)")
                .body_contains("RETURN anchor.id")
                .body_contains("\"limit\":2");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": [
                        "anchor.id",
                        "clip.id",
                        "clip.mime",
                        "clip.base64",
                        "clip.sample_rate",
                        "clip.channels",
                        "clip.captured_at",
                        "clip.occurred_at",
                        "clip.sensation_id"
                    ],
                    "data": [
                        {
                            "row": [
                                "audio:2",
                                "audio:1",
                                "audio/pcm;format=s16le;rate=16000",
                                "AAA=",
                                16000,
                                1,
                                "2026-05-05T12:34:56Z",
                                "2026-05-05T12:34:57Z",
                                "sensation:audio:1"
                            ]
                        },
                        {
                            "row": [
                                "audio:2",
                                "audio:2",
                                "audio/pcm;format=s16le;rate=16000",
                                "AQE=",
                                16000,
                                1,
                                "2026-05-05T12:34:58Z",
                                "2026-05-05T12:34:59Z",
                                "sensation:audio:2"
                            ]
                        }
                    ]
                }],
                "errors": []
            }));
        })
        .await;

    let window = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .latest_audio_clip_window_for_big_transcription(2)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(window.anchor_id, "audio:2");
    assert_eq!(window.clips.len(), 2);
    assert_eq!(window.clips[0].id, "audio:1");
    assert_eq!(
        window.clips[0].sensation_id.as_deref(),
        Some("sensation:audio:1")
    );
    assert_eq!(window.clips[1].id, "audio:2");
    assert_eq!(window.clips[1].clip.base64, "AQE=");
    assert_eq!(
        window.clips[1].sensation_id.as_deref(),
        Some("sensation:audio:2")
    );
    query.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_loads_latest_unprocessed_audio_clip_for_voice_recognition() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MATCH (a:GraphNode:AudioClip)")
                .body_contains("HAS_VOICE_RECOGNITION_RUN")
                .body_contains("OPTIONAL MATCH (s:GraphNode:Sensation)-[:OBSERVED]->(a)")
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
                        "a.occurred_at",
                        "s.id"
                    ],
                    "data": [{
                        "row": [
                            "audio:1",
                            "audio/pcm;format=s16le;rate=16000",
                            "AAA=",
                            16000,
                            1,
                            "2026-05-05T12:34:56Z",
                            "2026-05-05T12:34:57Z",
                            "sensation:audio:1"
                        ]
                    }]
                }],
                "errors": []
            }));
        })
        .await;

    let clip = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .latest_unprocessed_audio_clip_for_voice_recognition()
        .await
        .unwrap()
        .unwrap();

    assert_eq!(clip.id, "audio:1");
    assert_eq!(clip.clip.mime, "audio/pcm;format=s16le;rate=16000");
    assert_eq!(clip.clip.base64, "AAA=");
    assert_eq!(clip.clip.sample_rate, 16000);
    assert_eq!(clip.clip.channels, 1);
    assert_eq!(clip.sensation_id.as_deref(), Some("sensation:audio:1"));
    query.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_loads_latest_unprocessed_image_frame_for_face_recognition() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MATCH (i:GraphNode:Image)")
                .body_contains("HAS_FACE_RECOGNITION_RUN")
                .body_contains("OPTIONAL MATCH (s:GraphNode:Sensation)-[:OBSERVED]->(i)")
                .body_contains("ORDER BY observed_at DESC");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": [
                        "i.id",
                        "i.mime",
                        "i.base64",
                        "i.captured_at",
                        "i.occurred_at",
                        "s.id"
                    ],
                    "data": [{
                        "row": [
                            "image:1",
                            "image/jpeg",
                            "/9j/AA==",
                            "2026-05-05T12:34:56Z",
                            "2026-05-05T12:34:57Z",
                            "sensation:image:image:1:2026-05-05T12:34:56Z"
                        ]
                    }]
                }],
                "errors": []
            }));
        })
        .await;

    let frame = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .latest_unprocessed_image_frame_for_face_recognition()
        .await
        .unwrap()
        .unwrap();

    assert_eq!(frame.id, "image:1");
    assert_eq!(frame.image.mime, "image/jpeg");
    assert_eq!(frame.image.base64, "/9j/AA==");
    assert_eq!(
        frame.image.captured_at.as_deref(),
        Some("2026-05-05T12:34:56Z")
    );
    assert_eq!(frame.occurred_at.as_deref(), Some("2026-05-05T12:34:57Z"));
    assert_eq!(
        frame.sensation_id.as_deref(),
        Some("sensation:image:image:1:2026-05-05T12:34:56Z")
    );
    query.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_loads_latest_unprocessed_image_frame_for_scene_vectorization() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MATCH (i:GraphNode:Image)")
                .body_contains("HAS_SCENE_VECTORIZATION_RUN")
                .body_contains("SceneVectorizationRun")
                .body_contains("OPTIONAL MATCH (s:GraphNode:Sensation)-[:OBSERVED]->(i)")
                .body_contains("ORDER BY observed_at DESC");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": [
                        "i.id",
                        "i.mime",
                        "i.base64",
                        "i.captured_at",
                        "i.occurred_at",
                        "s.id"
                    ],
                    "data": [{
                        "row": [
                            "image:1",
                            "image/jpeg",
                            "/9j/AA==",
                            "2026-05-05T12:34:56Z",
                            "2026-05-05T12:34:57Z",
                            "sensation:image:image:1:2026-05-05T12:34:56Z"
                        ]
                    }]
                }],
                "errors": []
            }));
        })
        .await;

    let frame = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .latest_unprocessed_image_frame_for_scene_vectorization()
        .await
        .unwrap()
        .unwrap();

    assert_eq!(frame.id, "image:1");
    assert_eq!(frame.image.mime, "image/jpeg");
    assert_eq!(frame.image.base64, "/9j/AA==");
    assert_eq!(
        frame.image.captured_at.as_deref(),
        Some("2026-05-05T12:34:56Z")
    );
    assert_eq!(frame.occurred_at.as_deref(), Some("2026-05-05T12:34:57Z"));
    assert_eq!(
        frame.sensation_id.as_deref(),
        Some("sensation:image:image:1:2026-05-05T12:34:56Z")
    );
    query.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_loads_latest_unprocessed_image_frame_for_description() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MATCH (i:GraphNode:Image)")
                .body_contains("HAS_IMAGE_DESCRIPTION_RUN")
                .body_contains("ImageDescriptionRun")
                .body_contains("OPTIONAL MATCH (s:GraphNode:Sensation)-[:OBSERVED]->(i)")
                .body_contains("ORDER BY observed_at DESC");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": [
                        "i.id",
                        "i.mime",
                        "i.base64",
                        "i.captured_at",
                        "i.occurred_at",
                        "s.id"
                    ],
                    "data": [{
                        "row": [
                            "image:1",
                            "image/jpeg",
                            "/9j/AA==",
                            "2026-05-05T12:34:56Z",
                            "2026-05-05T12:34:57Z",
                            "sensation:image:image:1:2026-05-05T12:34:56Z"
                        ]
                    }]
                }],
                "errors": []
            }));
        })
        .await;

    let frame = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .latest_unprocessed_image_frame_for_description()
        .await
        .unwrap()
        .unwrap();

    assert_eq!(frame.id, "image:1");
    assert_eq!(frame.image.mime, "image/jpeg");
    assert_eq!(frame.image.base64, "/9j/AA==");
    assert_eq!(
        frame.sensation_id.as_deref(),
        Some("sensation:image:image:1:2026-05-05T12:34:56Z")
    );
    query.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_loads_latest_unprocessed_geolocation_for_vectorization() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MATCH (g:GraphNode:Geolocation)")
                .body_contains("HAS_GEOLOCATION_VECTOR")
                .body_contains("HAS_GEOLOCATION_VECTORIZATION_RUN")
                .body_contains("GeolocationVectorizationRun")
                .body_contains("OPTIONAL MATCH (s:GraphNode:Sensation)-[:OBSERVED]->(g)")
                .body_contains("ORDER BY observed_at DESC");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": [
                        "g.id",
                        "g.latitude",
                        "g.longitude",
                        "g.observed_at",
                        "g.occurred_at",
                        "s.id"
                    ],
                    "data": [{
                        "row": [
                            "geolocation:1",
                            37.7749,
                            -122.4194,
                            "2026-05-05T12:34:56Z",
                            "2026-05-05T12:34:57Z",
                            "sensation:geolocation:1"
                        ]
                    }]
                }],
                "errors": []
            }));
        })
        .await;

    let loc = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .latest_unprocessed_geolocation_for_vectorization()
        .await
        .unwrap()
        .unwrap();

    assert_eq!(loc.id, "geolocation:1");
    assert_eq!(loc.loc.latitude, 37.7749);
    assert_eq!(loc.loc.longitude, -122.4194);
    assert_eq!(loc.loc.observed_at.as_deref(), Some("2026-05-05T12:34:56Z"));
    assert_eq!(loc.occurred_at.as_deref(), Some("2026-05-05T12:34:57Z"));
    assert_eq!(loc.sensation_id.as_deref(), Some("sensation:geolocation:1"));
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
                .body_contains("WHERE EXISTS { MATCH (n)--(:GraphNode) }")
                .body_contains("MATCH (anchor)--(neighbor:GraphNode)")
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
                                "id": "face:1",
                                "labels": ["GraphNode", "Face"],
                                "properties": {
                                    "id": "face:1",
                                    "crop_mime": "image/jpeg",
                                    "crop_base64": "too-large-face"
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

    assert_eq!(snapshot.nodes.len(), 3);
    assert_eq!(snapshot.nodes[0].id, "image:1");
    assert_eq!(snapshot.nodes[0].labels, vec!["GraphNode", "Image"]);
    assert_eq!(snapshot.nodes[0].properties["mime"], "image/jpeg");
    assert!(snapshot.nodes[0].properties.get("base64").is_none());
    assert_eq!(snapshot.nodes[1].id, "face:1");
    assert_eq!(snapshot.nodes[1].properties["crop_mime"], "image/jpeg");
    assert!(snapshot.nodes[1].properties.get("crop_base64").is_none());
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
async fn neo4j_client_loads_graph_node_details_with_media_payload() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MATCH (n:GraphNode {id: $id})")
                .body_contains("\"id\":\"image:1\"");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": ["node", "relationships"],
                    "data": [{
                        "row": [
                            {
                                "id": "image:1",
                                "labels": ["GraphNode", "Image"],
                                "properties": {
                                    "id": "image:1",
                                    "mime": "image/png",
                                    "base64": "iVBORw0KGgo=",
                                    "embedding": [0.1, 0.2]
                                }
                            },
                            [{
                                "id": "5:abc:9",
                                "source": "sensation:1",
                                "target": "image:1",
                                "type": "OBSERVED",
                                "properties": {}
                            }]
                        ]
                    }]
                }],
                "errors": []
            }));
        })
        .await;

    let details = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .graph_node_details("image:1")
        .await
        .unwrap()
        .unwrap();

    assert_eq!(details.id, "image:1");
    assert_eq!(details.labels, vec!["GraphNode", "Image"]);
    assert_eq!(details.properties["mime"], "image/png");
    assert_eq!(details.properties["base64"], "iVBORw0KGgo=");
    assert!(details.properties.get("embedding").is_none());
    assert_eq!(details.relationships.len(), 1);
    assert_eq!(details.relationships[0].relationship_type, "OBSERVED");
    query.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_loads_speech_segment_audio_source() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MATCH (s:GraphNode:SpeechSegment {id: $id})")
                .body_contains("HAS_BIG_TRANSCRIPTION")
                .body_contains("DERIVED_FROM_AUDIO")
                .body_contains("\"id\":\"speech:1\"");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": [
                        "s.id",
                        "coalesce(s.text, \"\")",
                        "a.id",
                        "a.mime",
                        "a.base64",
                        "a.sample_rate",
                        "a.channels",
                        "clip_start_ms",
                        "clip_end_ms"
                    ],
                    "data": [{
                        "row": [
                            "speech:1",
                            "hello",
                            "audio:1",
                            "audio/pcm;format=s16le;rate=16000",
                            "AAAA",
                            16000,
                            1,
                            250,
                            550
                        ]
                    }]
                }],
                "errors": []
            }));
        })
        .await;

    let audio = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .graph_speech_segment_audio("speech:1")
        .await
        .unwrap()
        .unwrap();

    assert_eq!(audio.segment_id, "speech:1");
    assert_eq!(audio.text, "hello");
    assert_eq!(audio.audio_clip_id, "audio:1");
    assert_eq!(audio.mime, "audio/pcm;format=s16le;rate=16000");
    assert_eq!(audio.base64, "AAAA");
    assert_eq!(audio.sample_rate, 16000);
    assert_eq!(audio.channels, 1);
    assert_eq!(audio.start_ms, 250);
    assert_eq!(audio.end_ms, 550);
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
                .body_contains("AudioClip")
                .body_contains("Transcription")
                .body_contains("SpeechSegment")
                .body_contains("HAS_TRANSCRIPTION")
                .body_contains("DERIVED_FROM_AUDIO")
                .body_contains("HAS_SEGMENT")
                .body_contains("\"start_ms\":250")
                .body_contains("\"end_ms\":1250")
                .body_contains("2026-05-05T12:34:56.250+00:00")
                .matches(|req| {
                    req.body
                        .as_deref()
                        .and_then(|body| std::str::from_utf8(body).ok())
                        .is_some_and(|body| !body.contains("SEGMENT_OF"))
                });
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

#[tokio::test]
async fn neo4j_client_attaches_big_audio_transcription() {
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
                .body_contains("\"kind\":\"big\"")
                .body_contains("AudioClip")
                .body_contains("\"id\":\"audio:1\"")
                .body_contains("\"id\":\"audio:2\"")
                .body_contains("\"audio_clip_ids\":[\"audio:1\",\"audio:2\"]")
                .body_contains("HAS_BIG_TRANSCRIPTION")
                .body_contains("HAS_SEGMENT")
                .body_contains("DERIVED_FROM_AUDIO")
                .body_contains("\"from\":\"big-transcription:")
                .body_contains("PRODUCED")
                .body_contains("sensation:audio:1")
                .body_contains("sensation:audio:2")
                .body_contains("\"anchor\":true")
                .body_contains("\"source_index\":1")
                .body_contains("\"text\":\"hello there\"")
                .body_contains("\"transcript\":\"hello there\"");
            then.status(200).body(r#"{"results":[{}],"errors":[]}"#);
        })
        .await;

    Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .attach_big_audio_transcription(
            &[
                GraphAudioSourceSpan {
                    index: 0,
                    audio_clip_id: "audio:1".into(),
                    start_ms: 0,
                    end_ms: 1000,
                    occurred_at: Some("2026-05-05T12:34:56Z".into()),
                    ended_at: Some("2026-05-05T12:34:57Z".into()),
                    anchor: false,
                    sensation_id: Some("sensation:audio:1".into()),
                },
                GraphAudioSourceSpan {
                    index: 1,
                    audio_clip_id: "audio:2".into(),
                    start_ms: 1000,
                    end_ms: 2000,
                    occurred_at: Some("2026-05-05T12:34:58Z".into()),
                    ended_at: Some("2026-05-05T12:34:59Z".into()),
                    anchor: true,
                    sensation_id: Some("sensation:audio:2".into()),
                },
            ],
            "hello there",
            Some("2026-05-05T12:34:56Z"),
            Some("2026-05-05T12:34:58Z"),
            &[GraphSpeechSegment {
                index: 0,
                text: "hello there".into(),
                start_ms: 900,
                end_ms: 1300,
                occurred_at: Some("2026-05-05T12:34:56.900+00:00".into()),
                ended_at: Some("2026-05-05T12:34:57.300+00:00".into()),
            }],
        )
        .await
        .unwrap();

    constraint.assert_async().await;
    update.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_attaches_face_recognition() {
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
                .body_contains("FaceRecognitionRun")
                .body_contains("HAS_FACE_RECOGNITION_RUN")
                .body_contains("DETECTED_FACE")
                .body_contains("CONTAINS_FACE")
                .body_contains("HAS_FACE_VECTOR")
                .body_contains("qdrant:faces:point-1")
                .body_contains("sensation:image:1")
                .body_contains("\"face_count\":1")
                .body_contains("\"embedding_len\":512")
                .body_contains("\"detector\":\"face_id\"");
            then.status(200).body(r#"{"results":[{}],"errors":[]}"#);
        })
        .await;

    Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .attach_face_recognition(
            &GraphImageFrame {
                id: "image:1".into(),
                image: ImageData {
                    mime: "image/jpeg".into(),
                    base64: "/9j/AA==".into(),
                    captured_at: Some("2026-05-05T12:34:56Z".into()),
                },
                occurred_at: Some("2026-05-05T12:34:57Z".into()),
                sensation_id: Some("sensation:image:1".into()),
            },
            "face_id",
            &[GraphFaceDetection {
                index: 0,
                face_id: "face:1".into(),
                crop: ImageData {
                    mime: "image/jpeg".into(),
                    base64: "/9j/crop==".into(),
                    captured_at: Some("2026-05-05T12:34:56Z".into()),
                },
                vector_id: "point-1".into(),
                embedding_len: 512,
            }],
        )
        .await
        .unwrap();

    constraint.assert_async().await;
    update.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_attaches_scene_vectorization() {
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
                .body_contains("SceneVectorizationRun")
                .body_contains("HAS_SCENE_VECTORIZATION_RUN")
                .body_contains("HAS_SCENE_VECTOR")
                .body_contains("DERIVED_FROM")
                .body_contains("qdrant:scene_vectors:point-1")
                .body_contains("sensation:image:1")
                .body_contains("\"embedding_len\":512")
                .body_contains("\"model\":\"clip-test\"");
            then.status(200).body(r#"{"results":[{}],"errors":[]}"#);
        })
        .await;

    Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .attach_scene_vectorization(
            &GraphImageFrame {
                id: "image:1".into(),
                image: ImageData {
                    mime: "image/jpeg".into(),
                    base64: "/9j/AA==".into(),
                    captured_at: Some("2026-05-05T12:34:56Z".into()),
                },
                occurred_at: Some("2026-05-05T12:34:57Z".into()),
                sensation_id: Some("sensation:image:1".into()),
            },
            "clip-test",
            &GraphSceneVectorization {
                vector_id: "point-1".into(),
                embedding_len: 512,
            },
        )
        .await
        .unwrap();

    constraint.assert_async().await;
    update.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_attaches_image_description() {
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
                .body_contains("ImageDescriptionRun")
                .body_contains("ImageDescription")
                .body_contains("HAS_IMAGE_DESCRIPTION_RUN")
                .body_contains("HAS_IMAGE_DESCRIPTION")
                .body_contains("HAS_IMAGE_DESCRIPTION_VECTOR")
                .body_contains("qdrant:image_descriptions:point-1")
                .body_contains("sensation:image:1")
                .body_contains("\"embedding_len\":768")
                .body_contains("\"model\":\"vision-test\"")
                .body_contains("\"embedding_model\":\"embed-test\"");
            then.status(200).body(r#"{"results":[{}],"errors":[]}"#);
        })
        .await;

    Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .attach_image_description(
            &GraphImageFrame {
                id: "image:1".into(),
                image: ImageData {
                    mime: "image/jpeg".into(),
                    base64: "/9j/AA==".into(),
                    captured_at: Some("2026-05-05T12:34:56Z".into()),
                },
                occurred_at: Some("2026-05-05T12:34:57Z".into()),
                sensation_id: Some("sensation:image:1".into()),
            },
            "vision-test",
            "embed-test",
            &GraphImageDescription {
                description_id: "image-description-text:image:1".into(),
                text: "I see a test frame.".into(),
                vector_id: "point-1".into(),
                embedding_len: 768,
            },
        )
        .await
        .unwrap();

    constraint.assert_async().await;
    update.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_attaches_geolocation_vectorization() {
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
                .body_contains("GeolocationVectorizationRun")
                .body_contains("HAS_GEOLOCATION_VECTORIZATION_RUN")
                .body_contains("HAS_GEOLOCATION_VECTOR")
                .body_contains("PROCESSED_GEOLOCATION")
                .body_contains("qdrant:geolocations:point-1")
                .body_contains("sensation:geolocation:1")
                .body_contains("\"embedding_len\":3")
                .body_contains("\"model\":\"earth-unit-sphere/v1\"");
            then.status(200).body(r#"{"results":[{}],"errors":[]}"#);
        })
        .await;

    Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .attach_geolocation_vectorization(
            &GraphGeolocation {
                id: "geolocation:1".into(),
                loc: GeoLoc {
                    latitude: 37.7749,
                    longitude: -122.4194,
                    observed_at: Some("2026-05-05T12:34:56Z".into()),
                },
                occurred_at: Some("2026-05-05T12:34:57Z".into()),
                sensation_id: Some("sensation:geolocation:1".into()),
            },
            "earth-unit-sphere/v1",
            "point-1",
            3,
        )
        .await
        .unwrap();

    constraint.assert_async().await;
    update.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_attaches_vector_clusters() {
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
                .body_contains("ClusterDiscoveryRun")
                .body_contains("Cluster")
                .body_contains("PRODUCED_CLUSTER")
                .body_contains("HAS_CLUSTER_MEMBER")
                .body_contains("MEMBER_OF_CLUSTER")
                .body_contains("qdrant:memories:point-1")
                .body_contains("qdrant:memories:point-2")
                .body_contains("\"cluster_count\":1")
                .body_contains("\"source_count\":3")
                .body_contains("\"member_count\":2")
                .body_contains("\"algorithm\":\"cosine-threshold-components/v1\"");
            then.status(200).body(r#"{"results":[{}],"errors":[]}"#);
        })
        .await;

    Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .attach_vector_clusters(
            "memories",
            "cosine-threshold-components/v1",
            0.9,
            2,
            3,
            &[VectorCluster {
                cluster_id: "cluster:1".into(),
                collection: "memories".into(),
                threshold: 0.9,
                centroid: vec![0.95, 0.05],
                mean_similarity: 0.95,
                members: vec![
                    VectorClusterMember {
                        point_id: "point-1".into(),
                        average_similarity: 0.95,
                    },
                    VectorClusterMember {
                        point_id: "point-2".into(),
                        average_similarity: 0.95,
                    },
                ],
            }],
        )
        .await
        .unwrap();

    constraint.assert_async().await;
    update.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_loads_latest_timeline_window_for_combobulation() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MATCH (anchor:GraphNode)")
                .body_contains("INCLUDED_IN_COMBOBULATION")
                .body_contains("duration({seconds: $seconds})")
                .body_contains("RETURN anchor.id, anchor_at, n.id, labels(n), text, occurred_at")
                .body_contains("\"seconds\":30")
                .body_contains("\"limit\":80");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": ["anchor.id", "anchor_at", "n.id", "labels(n)", "text", "occurred_at"],
                    "data": [
                        {"row": [
                            "speech:2",
                            "2026-05-05T12:35:00Z",
                            "speech:1",
                            ["GraphNode", "SpeechSegment"],
                            "speech: hello",
                            "2026-05-05T12:34:56Z"
                        ]},
                        {"row": [
                            "speech:2",
                            "2026-05-05T12:35:00Z",
                            "speech:2",
                            ["GraphNode", "SpeechSegment"],
                            "speech: there",
                            "2026-05-05T12:35:00Z"
                        ]}
                    ]
                }],
                "errors": []
            }));
        })
        .await;

    let window = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .latest_timeline_window_for_combobulation(30, 80)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(window.anchor_id, "speech:2");
    assert_eq!(window.anchor_at, "2026-05-05T12:35:00Z");
    assert_eq!(window.items.len(), 2);
    assert_eq!(window.items[0].id, "speech:1");
    assert_eq!(window.items[0].labels, ["GraphNode", "SpeechSegment"]);
    assert_eq!(window.items[1].text, "speech: there");
    query.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_attaches_combobulation() {
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
                .body_contains("CombobulationRun")
                .body_contains("Awareness")
                .body_contains("INCLUDED_IN_COMBOBULATION")
                .body_contains("HAS_MEMORY_VECTOR")
                .body_contains("qdrant:memories:point-1")
                .body_contains("\"model\":\"wit-test\"")
                .body_contains("\"embedding_model\":\"embed-test\"")
                .body_contains("\"source_count\":2");
            then.status(200).body(r#"{"results":[{}],"errors":[]}"#);
        })
        .await;

    Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .attach_combobulation(
            &GraphTimelineWindow {
                anchor_id: "speech:2".into(),
                anchor_at: "2026-05-05T12:35:00Z".into(),
                items: vec![
                    GraphTimelineItem {
                        id: "speech:1".into(),
                        labels: vec!["GraphNode".into(), "SpeechSegment".into()],
                        text: "speech: hello".into(),
                        occurred_at: "2026-05-05T12:34:56Z".into(),
                    },
                    GraphTimelineItem {
                        id: "speech:2".into(),
                        labels: vec!["GraphNode".into(), "SpeechSegment".into()],
                        text: "speech: there".into(),
                        occurred_at: "2026-05-05T12:35:00Z".into(),
                    },
                ],
            },
            "wit-test",
            "embed-test",
            &GraphAwareness {
                awareness_id: "awareness:speech:2".into(),
                text: "I hear someone greeting me.".into(),
                vector_id: "point-1".into(),
                embedding_len: 768,
            },
        )
        .await
        .unwrap();

    constraint.assert_async().await;
    update.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_attaches_voice_recognition() {
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
                .body_contains("VoiceRecognitionRun")
                .body_contains("VoiceSignature")
                .body_contains("VoiceSample")
                .body_contains("HAS_VOICE_RECOGNITION_RUN")
                .body_contains("HAS_VOICE_VECTOR")
                .body_contains("qdrant:voices:point-1")
                .body_contains("sensation:audio:1")
                .body_contains("fundamental_frequency")
                .body_contains("quality_score")
                .body_contains("\"model\":\"voxudio\"");
            then.status(200).body(r#"{"results":[{}],"errors":[]}"#);
        })
        .await;

    Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .attach_voice_recognition(
            &GraphVoiceClip {
                id: "audio:1".into(),
                clip: psyche::AudioClip {
                    mime: "audio/pcm;format=s16le;rate=16000".into(),
                    base64: "AAA=".into(),
                    sample_rate: 16000,
                    channels: 1,
                    transcript: None,
                    captured_at: Some("2026-05-05T12:34:56Z".into()),
                },
                occurred_at: Some("2026-05-05T12:34:57Z".into()),
                sensation_id: Some("sensation:audio:1".into()),
            },
            "voxudio",
            &GraphVoiceRecognition {
                signature: GraphVoiceSignature {
                    user_id: "speaker:1".into(),
                    fundamental_frequency: 150.0,
                    frequency_range: (100.0, 300.0),
                    formant_frequencies: vec![800.0, 1200.0, 2500.0],
                    speech_rate: 4.5,
                    mfcc_signature: vec![0.1, 0.2],
                    spectral_centroid: 1500.0,
                    jitter: 0.5,
                    shimmer: 3.0,
                    harmonic_to_noise_ratio: 20.0,
                    sample_count: 1,
                    last_updated: Utc::now(),
                    tags: vec!["voice".into()],
                },
                sample: GraphVoiceSample {
                    id: "voice-sample:1".into(),
                    user_id: "speaker:1".into(),
                    duration_ms: 2000,
                    sample_rate: 16000,
                    fundamental_frequency: 150.0,
                    formant_frequencies: vec![800.0, 1200.0, 2500.0],
                    mfcc: vec![0.1, 0.2],
                    quality_score: 0.9,
                    timestamp: Utc::now(),
                },
                vector_id: "point-1".into(),
                embedding_len: 256,
            },
        )
        .await
        .unwrap();

    constraint.assert_async().await;
    update.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_attaches_skipped_voice_recognition() {
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
                .body_contains("VoiceRecognitionRun")
                .body_contains("HAS_VOICE_RECOGNITION_RUN")
                .body_contains("PROCESSED_AUDIO")
                .body_contains("sensation:audio:1")
                .body_contains("\"status\":\"skipped\"")
                .body_contains("\"reason\":\"audio clip too short\"")
                .body_contains("\"model\":\"voxudio\"");
            then.status(200).body(r#"{"results":[{}],"errors":[]}"#);
        })
        .await;

    Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .attach_skipped_voice_recognition(
            &GraphVoiceClip {
                id: "audio:1".into(),
                clip: psyche::AudioClip {
                    mime: "audio/pcm;format=s16le;rate=16000".into(),
                    base64: "AAA=".into(),
                    sample_rate: 16000,
                    channels: 1,
                    transcript: None,
                    captured_at: Some("2026-05-05T12:34:56Z".into()),
                },
                occurred_at: Some("2026-05-05T12:34:57Z".into()),
                sensation_id: Some("sensation:audio:1".into()),
            },
            "voxudio",
            "audio clip too short",
        )
        .await
        .unwrap();

    constraint.assert_async().await;
    update.assert_async().await;
}

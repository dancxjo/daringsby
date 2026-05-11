use chrono::Utc;
use httpmock::{Method::POST, MockServer};
use psyche::{
    AudioClip, GeoLoc, GraphAudioClip, GraphAudioSourceSpan, GraphAwareness, GraphClusterItem,
    GraphClusterTheme, GraphConsolidatedSpeechCandidate, GraphConsolidatedSpeechSource,
    GraphFaceDetection, GraphFaceIdentityLabel, GraphGeolocation, GraphImageDescription,
    GraphImageFrame, GraphSceneVectorization, GraphSpeechSegment, GraphTimelineItem,
    GraphTimelineWindow, GraphVoiceClip, GraphVoiceIdentityLabel, GraphVoiceRecognition,
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
async fn neo4j_client_counts_non_raw_graph_nodes() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MATCH (n:GraphNode)")
                .body_contains("NOT n:Sensation OR coalesce(n.derived, false) = true")
                .body_contains("NOT n:AudioClip")
                .body_contains("NOT n:Image")
                .body_contains("RETURN count(n)");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": ["count(n)"],
                    "data": [{
                        "row": [12]
                    }]
                }],
                "errors": []
            }));
        })
        .await;

    let count = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .count_non_raw_graph_nodes()
        .await
        .unwrap();

    assert_eq!(count, 12);
    query.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_detach_deletes_non_raw_graph_nodes_with_batch_limit() {
    let server = MockServer::start_async().await;
    let delete = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MATCH (n:GraphNode)")
                .body_contains("NOT n:Sensation OR coalesce(n.derived, false) = true")
                .body_contains("NOT n:AudioClip")
                .body_contains("NOT n:Image")
                .body_contains("LIMIT $limit")
                .body_contains("DETACH DELETE node")
                .body_contains("\"limit\":2");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": ["deleted_count"],
                    "data": [{
                        "row": [0]
                    }]
                }],
                "errors": []
            }));
        })
        .await;

    let deleted = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .detach_delete_non_raw_graph_nodes(2)
        .await
        .unwrap();

    assert_eq!(deleted, 0);
    delete.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_counts_audio_clip_transcript_properties() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MATCH (a:GraphNode:AudioClip)")
                .body_contains("a.transcript IS NOT NULL")
                .body_contains("RETURN count(a)");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": ["count(a)"],
                    "data": [{
                        "row": [7]
                    }]
                }],
                "errors": []
            }));
        })
        .await;

    let count = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .count_audio_clip_transcript_properties()
        .await
        .unwrap();

    assert_eq!(count, 7);
    query.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_clears_audio_clip_transcript_properties() {
    let server = MockServer::start_async().await;
    let clear = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MATCH (a:GraphNode:AudioClip)")
                .body_contains("a.transcript IS NOT NULL")
                .body_contains("REMOVE clip.transcript, clip.transcribed_at")
                .body_contains("RETURN cleared_count");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": ["cleared_count"],
                    "data": [{
                        "row": [7]
                    }]
                }],
                "errors": []
            }));
        })
        .await;

    let cleared = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .clear_audio_clip_transcript_properties()
        .await
        .unwrap();

    assert_eq!(cleared, 7);
    clear.assert_async().await;
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
async fn neo4j_client_loads_latest_big_transcription_for_speech_consolidation() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MATCH (t:GraphNode:Transcription)")
                .body_contains("HAS_CONSOLIDATED_AUDIO")
                .body_contains("HAS_BIG_TRANSCRIPTION")
                .body_contains("HAS_TRANSCRIPTION")
                .body_contains("min_source_count");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": [
                        "t.id",
                        "transcript",
                        "t.source_started_at",
                        "t.source_ended_at",
                        "sources"
                    ],
                    "data": [{
                        "row": [
                            "big:1",
                            "hello there",
                            "2026-05-05T12:34:56Z",
                            "2026-05-05T12:34:58Z",
                            [
                                {
                                    "index": 0,
                                    "id": "audio:1",
                                    "mime": "audio/pcm;format=s16le;rate=16000",
                                    "base64": "AAA=",
                                    "sample_rate": 16000,
                                    "channels": 1,
                                    "transcript": "hello",
                                    "captured_at": "2026-05-05T12:34:56Z",
                                    "occurred_at": "2026-05-05T12:34:56Z",
                                    "sensation_id": "sensation:audio:1",
                                    "start_ms": 0,
                                    "end_ms": 1000,
                                    "transcription_ids": ["old:1"]
                                },
                                {
                                    "index": 1,
                                    "id": "audio:2",
                                    "mime": "audio/pcm;format=s16le;rate=16000",
                                    "base64": "AQE=",
                                    "sample_rate": 16000,
                                    "channels": 1,
                                    "captured_at": "2026-05-05T12:34:57Z",
                                    "start_ms": 1000,
                                    "end_ms": 2000,
                                    "transcription_ids": ["old:2"]
                                }
                            ]
                        ]
                    }]
                }],
                "errors": []
            }));
        })
        .await;

    let candidate = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .latest_big_transcription_for_speech_consolidation(2)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(candidate.transcription_id, "big:1");
    assert_eq!(candidate.transcript, "hello there");
    assert_eq!(candidate.sources.len(), 2);
    assert_eq!(candidate.sources[0].clip.id, "audio:1");
    assert_eq!(candidate.sources[0].transcription_ids, ["old:1"]);
    assert_eq!(candidate.sources[1].start_ms, 1000);
    assert_eq!(
        candidate.sources[0].clip.sensation_id.as_deref(),
        Some("sensation:audio:1")
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
                .body_contains("n.source_started_at")
                .body_contains("n.source_captured_at")
                .body_contains("n.source_ended_at")
                .body_contains("MATCH (anchor)--(neighbor:GraphNode)")
                .body_contains("LIMIT $limit")
                .body_contains("candidate_nodes[..$limit] AS nodes")
                .body_contains("ELSE n {.*, occurred_at: event_at}")
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
                                "labels": ["GraphNode", "FaceInstance"],
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
                .body_contains("ELSE n {.*, occurred_at: event_at}")
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
async fn neo4j_client_loads_face_node_details_with_linked_face_images() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("face_images")
                .body_contains("HAS_FACE_VECTOR")
                .body_contains("MATCHED_FACE")
                .body_contains("\"id\":\"face:known\"");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": ["node", "relationships"],
                    "data": [{
                        "row": [
                            {
                                "id": "face:known",
                                "labels": ["GraphNode", "Cluster", "Face"],
                                "properties": {
                                    "id": "face:known",
                                    "kind": "face",
                                    "face_images": [{
                                        "id": "face-instance:1",
                                        "source_image_id": "image:1",
                                        "mime": "image/png",
                                        "base64": "iVBORw0KGgo=",
                                        "captured_at": "2026-05-07T12:00:00Z"
                                    }]
                                }
                            },
                            []
                        ]
                    }]
                }],
                "errors": []
            }));
        })
        .await;

    let details = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .graph_node_details("face:known")
        .await
        .unwrap()
        .unwrap();

    assert_eq!(details.id, "face:known");
    assert_eq!(details.labels, vec!["GraphNode", "Cluster", "Face"]);
    assert_eq!(
        details.properties["face_images"][0]["id"],
        "face-instance:1"
    );
    assert_eq!(
        details.properties["face_images"][0]["base64"],
        "iVBORw0KGgo="
    );
    query.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_loads_movie_export_media() {
    let server = MockServer::start_async().await;
    let latest_query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("GraphNode:Image")
                .body_contains("GraphNode:SpeechSegment");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": ["observed_at"],
                    "data": [{ "row": ["2026-05-07T12:01:30Z"] }]
                }],
                "errors": []
            }));
        })
        .await;
    let image_query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MATCH (i:GraphNode:Image)")
                .body_contains("datetime(observed_at) >= datetime($from)")
                .body_contains("\"from\":\"2026-05-07T12:00:00+00:00\"")
                .body_contains("\"to\":\"2026-05-07T12:01:30+00:00\"");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": ["i.id", "i.mime", "i.base64", "i.captured_at", "observed_at", "s.id"],
                    "data": [{ "row": [
                        "image:1",
                        "image/png",
                        "iVBORw0KGgo=",
                        "2026-05-07T12:00:00Z",
                        "2026-05-07T12:00:00Z",
                        "sensation:1"
                    ] }]
                }],
                "errors": []
            }));
        })
        .await;
    let prior_image_query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MATCH (i:GraphNode:Image)")
                .body_contains("datetime(observed_at) < datetime($before)")
                .body_contains("\"before\":\"2026-05-07T12:00:00+00:00\"");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": ["i.id", "i.mime", "i.base64", "i.captured_at", "observed_at", "s.id"],
                    "data": [{ "row": [
                        "image:0",
                        "image/jpeg",
                        "/9j/AA==",
                        null,
                        "2026-05-07T11:59:59Z",
                        null
                    ] }]
                }],
                "errors": []
            }));
        })
        .await;
    let speech_query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MATCH (speech:GraphNode:SpeechSegment)")
                .body_contains("datetime(started_at) <= datetime($to)");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": ["speech.id", "text", "start_ms", "end_ms", "started_at", "ended_at"],
                    "data": [{ "row": [
                        "speech:1",
                        "hello",
                        250,
                        1250,
                        "2026-05-07T12:00:02Z",
                        "2026-05-07T12:00:03Z"
                    ] }]
                }],
                "errors": []
            }));
        })
        .await;

    let graph = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into());
    let from = chrono::DateTime::parse_from_rfc3339("2026-05-07T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let to = chrono::DateTime::parse_from_rfc3339("2026-05-07T12:01:30Z")
        .unwrap()
        .with_timezone(&Utc);

    assert_eq!(
        graph.latest_movie_timestamp().await.unwrap().unwrap(),
        "2026-05-07T12:01:30Z"
    );
    let frames = graph.movie_image_frames(from, to).await.unwrap();
    assert_eq!(frames[0].id, "image:1");
    assert_eq!(frames[0].image.mime, "image/png");
    assert_eq!(frames[0].occurred_at, "2026-05-07T12:00:00Z");
    let prior = graph
        .latest_movie_image_frame_before(from)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(prior.id, "image:0");
    let speech = graph.movie_speech_segments(from, to).await.unwrap();
    assert_eq!(speech[0].id, "speech:1");
    assert_eq!(speech[0].text, "hello");
    assert_eq!(speech[0].start_ms, 250);
    assert_eq!(speech[0].ended_at.as_deref(), Some("2026-05-07T12:00:03Z"));

    latest_query.assert_async().await;
    image_query.assert_async().await;
    prior_image_query.assert_async().await;
    speech_query.assert_async().await;
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
                .body_contains("source_rel.clip_start_ms")
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
                .body_contains("\"source_captured_at\":\"2026-05-05T12:34:56Z\"")
                .body_contains("\"source_started_at\":\"2026-05-05T12:34:56Z\"")
                .body_contains("\"source_ended_at\":\"2026-05-05T12:34:57.250+00:00\"")
                .body_contains("\"occurred_at\":\"2026-05-05T12:34:56Z\"")
                .body_contains("\"kind\":\"transcription\"")
                .body_contains("\"derived\":true")
                .body_contains("\"how\":\"I heard: hello there.\"")
                .body_contains("\"source_sensation_ids\":[\"sensation:audio:1\"]")
                .body_contains("\"from\":\"sensation:audio:1\"")
                .body_contains("\"to\":\"sensation:audio:1\"")
                .body_contains("SpeechSegment")
                .body_contains("HAS_TRANSCRIPTION")
                .body_contains("DERIVED_FROM_AUDIO")
                .body_contains("DERIVED_FROM")
                .body_contains("PRODUCED")
                .body_contains("OBSERVED")
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
            Some("sensation:audio:1"),
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
                .body_contains("\"source_started_at\":\"2026-05-05T12:34:56Z\"")
                .body_contains("\"source_ended_at\":\"2026-05-05T12:34:58Z\"")
                .body_contains("\"occurred_at\":\"2026-05-05T12:34:56Z\"")
                .body_contains("HAS_BIG_TRANSCRIPTION")
                .body_contains("HAS_SEGMENT")
                .body_contains("DERIVED_FROM_AUDIO")
                .body_contains("\"from\":\"big-transcription:")
                .body_contains("PRODUCED")
                .body_contains("sensation:audio:1")
                .body_contains("sensation:audio:2")
                .body_contains("\"anchor\":true")
                .body_contains("\"source_index\":1")
                .body_contains("\"clip_start_ms\":900")
                .body_contains("\"clip_end_ms\":1000")
                .body_contains("\"clip_start_ms\":0")
                .body_contains("\"clip_end_ms\":300")
                .body_contains("\"source_start_ms\":1000")
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
async fn neo4j_client_consolidates_big_audio_transcription() {
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
                .body_contains("\"id\":\"audio:fused\"")
                .body_contains("\"transcript\":\"hello there\"")
                .body_contains("HAS_CONSOLIDATED_AUDIO")
                .body_contains("HAS_BIG_TRANSCRIPTION")
                .body_contains("DERIVED_FROM_AUDIO")
                .body_contains("clip_start_ms = start_ms")
                .body_contains("old:1")
                .body_contains("old:2")
                .body_contains("DETACH DELETE segment")
                .body_contains("DETACH DELETE old")
                .body_contains("DETACH DELETE a")
                .body_contains("DETACH DELETE s");
            then.status(200).body(r#"{"results":[{}],"errors":[]}"#);
        })
        .await;
    let candidate = GraphConsolidatedSpeechCandidate {
        transcription_id: "big:1".into(),
        transcript: "hello there".into(),
        source_started_at: Some("2026-05-05T12:34:56Z".into()),
        source_ended_at: Some("2026-05-05T12:34:58Z".into()),
        sources: vec![
            GraphConsolidatedSpeechSource {
                index: 0,
                clip: GraphAudioClip {
                    id: "audio:1".into(),
                    clip: AudioClip {
                        mime: "audio/pcm;format=s16le;rate=16000".into(),
                        base64: "AAA=".into(),
                        sample_rate: 16_000,
                        channels: 1,
                        transcript: Some("hello".into()),
                        captured_at: Some("2026-05-05T12:34:56Z".into()),
                    },
                    occurred_at: None,
                    sensation_id: Some("sensation:audio:1".into()),
                },
                start_ms: 0,
                end_ms: 1000,
                transcription_ids: vec!["old:1".into()],
            },
            GraphConsolidatedSpeechSource {
                index: 1,
                clip: GraphAudioClip {
                    id: "audio:2".into(),
                    clip: AudioClip {
                        mime: "audio/pcm;format=s16le;rate=16000".into(),
                        base64: "AQE=".into(),
                        sample_rate: 16_000,
                        channels: 1,
                        transcript: Some("there".into()),
                        captured_at: Some("2026-05-05T12:34:57Z".into()),
                    },
                    occurred_at: None,
                    sensation_id: Some("sensation:audio:2".into()),
                },
                start_ms: 1000,
                end_ms: 2000,
                transcription_ids: vec!["old:2".into(), "old:1".into()],
            },
        ],
    };
    let fused = AudioClip {
        mime: "audio/wav".into(),
        base64: "UklGRg==".into(),
        sample_rate: 16_000,
        channels: 1,
        transcript: Some("hello there".into()),
        captured_at: Some("2026-05-05T12:34:56Z".into()),
    };

    let report = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .consolidate_big_audio_transcription(&candidate, "audio:fused", &fused, 2000, true)
        .await
        .unwrap();

    assert_eq!(report.transcription_id, "big:1");
    assert_eq!(report.consolidated_audio_clip_id, "audio:fused");
    assert_eq!(report.source_audio_clip_ids, ["audio:1", "audio:2"]);
    assert_eq!(report.deleted_transcription_ids, ["old:1", "old:2"]);
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
                .body_contains("FaceInstance")
                .body_contains("HAS_FACE_RECOGNITION_RUN")
                .body_contains("DETECTED_FACE")
                .body_contains("CONTAINS_FACE")
                .body_contains("HAS_FACE_VECTOR")
                .body_contains("qdrant:faces:point-1")
                .body_contains("sensation:image:1")
                .body_contains("\"face_count\":1")
                .body_contains("\"kind\":\"face_recognition\"")
                .body_contains("\"derived\":true")
                .body_contains(format!(
                    "\"how\":\"{}\"",
                    psyche::face_count_sensation_text(1)
                ))
                .body_contains(format!(
                    "\"how\":\"{}\"",
                    psyche::face_familiarity_sensation_text(false)
                ))
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
                recognition: None,
            }],
        )
        .await
        .unwrap();

    constraint.assert_async().await;
    update.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_attaches_face_recognition_sensation_for_zero_faces() {
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
                .body_contains("Sensation")
                .body_contains("\"face_count\":0")
                .body_contains("\"kind\":\"face_recognition\"")
                .body_contains("\"derived\":true")
                .body_contains(format!(
                    "\"how\":\"{}\"",
                    psyche::face_count_sensation_text(0)
                ))
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
            &[],
        )
        .await
        .unwrap();

    constraint.assert_async().await;
    update.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_attaches_per_face_identity_sensation() {
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
                .body_contains("\"kind\":\"face_identity\"")
                .body_contains(format!(
                    "\"how\":\"{}\"",
                    psyche::face_familiarity_sensation_text(true)
                ))
                .body_contains("\"matched_face_id\":\"cluster:face:1\"")
                .body_contains("\"identity_name\":\"Anna\"")
                .body_contains("\"nearest_face_vector_id\":\"known-point\"")
                .body_contains("MATCHED_FACE")
                .body_contains("RECOGNIZED_AS")
                .body_contains("MATCHED_NEAREST_VECTOR");
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
            &[
                GraphFaceDetection {
                    index: 0,
                    face_id: "face:1".into(),
                    crop: ImageData {
                        mime: "image/jpeg".into(),
                        base64: "/9j/crop==".into(),
                        captured_at: Some("2026-05-05T12:34:56Z".into()),
                    },
                    vector_id: "point-1".into(),
                    embedding_len: 512,
                    recognition: Some(psyche::GraphFaceMatch {
                        face_id: "cluster:face:1".into(),
                        identity: Some("Anna".into()),
                        nearest_vector_id: "known-point".into(),
                        score: 0.93,
                    }),
                },
                GraphFaceDetection {
                    index: 1,
                    face_id: "face:2".into(),
                    crop: ImageData {
                        mime: "image/jpeg".into(),
                        base64: "/9j/crop2==".into(),
                        captured_at: Some("2026-05-05T12:34:56Z".into()),
                    },
                    vector_id: "point-2".into(),
                    embedding_len: 512,
                    recognition: Some(psyche::GraphFaceMatch {
                        face_id: "cluster:face:2".into(),
                        identity: None,
                        nearest_vector_id: "known-point-2".into(),
                        score: 0.91,
                    }),
                },
            ],
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
async fn neo4j_client_loads_vector_cluster_items() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("UNWIND $vector_ids AS vector_id")
                .body_contains("HAS_MEMORY_VECTOR")
                .body_contains("MATCH (owner)-[rel]-(neighbor:GraphNode)")
                .body_contains("coalesce(head([label IN labels(neighbor) WHERE label")
                .body_contains("WITH vector_id, owner, text, stimulus_texts, edge_texts, neighbor_texts")
                .body_contains("RETURN vector_id, owner.id, labels(owner), text, stimulus_texts, edge_texts, neighbor_texts")
                .body_contains("qdrant:memories:point-1")
                .body_contains("\"limit\":10");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": ["vector_id", "owner.id", "labels(owner)", "text", "stimulus_texts", "edge_texts", "neighbor_texts"],
                    "data": [{
                        "row": [
                            "qdrant:memories:point-1",
                            "impression:1",
                            ["GraphNode", "Impression"],
                            "impression: coffee is brewing",
                            ["text: coffee beans"],
                            ["-[:HAS_STIMULUS]-> stimulus:1"],
                            ["TextObservation text: coffee beans"]
                        ]
                    }]
                }],
                "errors": []
            }));
        })
        .await;

    let items = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .vector_cluster_items("memories", &["point-1".into()], 10)
        .await
        .unwrap();

    assert_eq!(
        items,
        vec![GraphClusterItem {
            vector_id: "qdrant:memories:point-1".into(),
            node_id: "impression:1".into(),
            labels: vec!["GraphNode".into(), "Impression".into()],
            text: "impression: coffee is brewing".into(),
            stimuli: vec!["text: coffee beans".into()],
            edges: vec!["-[:HAS_STIMULUS]-> stimulus:1".into()],
            neighbors: vec!["TextObservation text: coffee beans".into()],
        }]
    );
    query.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_loads_face_identity_for_vector_neighbor() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MATCH (v:GraphNode:Vector {id: $vector_id})")
                .body_contains(
                    "MATCH (v)-[:MEMBER_OF_CLUSTER|HAS_CLUSTER_MEMBER]-(face:GraphNode:Face)",
                )
                .body_contains("candidate:Identity")
                .body_contains("qdrant:faces:point-1");
            then.status(200)
                .body(r#"{"results":[{"data":[{"row":["cluster:face:1","Anna"]}]}],"errors":[]}"#);
        })
        .await;

    let identity = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .face_identity_for_vector_neighbor("point-1")
        .await
        .unwrap()
        .unwrap();

    assert_eq!(identity.face_id, "cluster:face:1");
    assert_eq!(identity.identity.as_deref(), Some("Anna"));
    query.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_checks_face_cluster_identity_run_presence() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("HAS_FACE_IDENTITY_RUN")
                .body_contains("cluster:face:1");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": ["EXISTS"],
                    "data": [{"row": [true]}]
                }],
                "errors": []
            }));
        })
        .await;

    let has_identity_run = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .face_cluster_has_identity_run("cluster:face:1")
        .await
        .unwrap();

    assert!(has_identity_run);
    query.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_loads_voice_identity_for_vector_neighbor() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MATCH (v:GraphNode:Vector {id: $vector_id})")
                .body_contains(
                    "MATCH (v)-[:MEMBER_OF_CLUSTER|HAS_CLUSTER_MEMBER]-(voice:GraphNode:Voice)",
                )
                .body_contains("candidate:Identity")
                .body_contains("qdrant:voices:point-1");
            then.status(200)
                .body(r#"{"results":[{"data":[{"row":["cluster:voice:1","Anna"]}]}],"errors":[]}"#);
        })
        .await;

    let identity = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .voice_identity_for_vector_neighbor("point-1")
        .await
        .unwrap()
        .unwrap();

    assert_eq!(identity.voice_id, "cluster:voice:1");
    assert_eq!(identity.identity.as_deref(), Some("Anna"));
    query.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_checks_voice_cluster_identity_run_presence() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("HAS_VOICE_IDENTITY_RUN")
                .body_contains("cluster:voice:1");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": ["EXISTS"],
                    "data": [{"row": [true]}]
                }],
                "errors": []
            }));
        })
        .await;

    let has_identity_run = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .voice_cluster_has_identity_run("cluster:voice:1")
        .await
        .unwrap();

    assert!(has_identity_run);
    query.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_checks_vector_cluster_theme_presence() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("HAS_THEME")
                .body_contains("cluster:1");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": ["EXISTS"],
                    "data": [{"row": [true]}]
                }],
                "errors": []
            }));
        })
        .await;

    let has_theme = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .vector_cluster_has_theme("cluster:1")
        .await
        .unwrap();

    assert!(has_theme);
    query.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_attaches_vector_cluster_theme() {
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
                .body_contains("ClusterThemeRun")
                .body_contains("Theme")
                .body_contains("HAS_THEME")
                .body_contains("THEME_OF")
                .body_contains("DERIVED_FROM_VECTOR")
                .body_contains("coffee rituals")
                .body_contains("qdrant:memories:point-1");
            then.status(200).body(r#"{"results":[{}],"errors":[]}"#);
        })
        .await;

    let cluster = VectorCluster {
        cluster_id: "cluster:1".into(),
        collection: "memories".into(),
        threshold: 0.9,
        centroid: vec![0.95, 0.05],
        mean_similarity: 0.95,
        members: vec![VectorClusterMember {
            point_id: "point-1".into(),
            average_similarity: 0.95,
        }],
    };
    let items = vec![GraphClusterItem {
        vector_id: "qdrant:memories:point-1".into(),
        node_id: "impression:1".into(),
        labels: vec!["Impression".into()],
        text: "impression: coffee is brewing".into(),
        stimuli: Vec::new(),
        edges: Vec::new(),
        neighbors: Vec::new(),
    }];
    let theme = GraphClusterTheme {
        theme_id: "theme:cluster:1".into(),
        text: "coffee rituals".into(),
    };

    Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .attach_vector_cluster_theme(&cluster, "gpt-oss", &items, &theme)
        .await
        .unwrap();

    constraint.assert_async().await;
    update.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_attaches_face_identity() {
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
                .body_contains("FaceIdentityRun")
                .body_contains("Identity")
                .body_contains("Person")
                .body_contains("HAS_FACE_IDENTITY_RUN")
                .body_contains("HAS_IDENTITY")
                .body_contains("IDENTITY_OF")
                .body_contains("USED_CONTEXT")
                .body_contains("Anna")
                .body_contains("qdrant:faces:point-1");
            then.status(200).body(r#"{"results":[{}],"errors":[]}"#);
        })
        .await;

    let cluster = VectorCluster {
        cluster_id: "cluster:face:1".into(),
        collection: "faces".into(),
        threshold: 0.9,
        centroid: vec![0.95, 0.05],
        mean_similarity: 0.95,
        members: vec![VectorClusterMember {
            point_id: "point-1".into(),
            average_similarity: 0.95,
        }],
    };
    let items = vec![GraphClusterItem {
        vector_id: "qdrant:faces:point-1".into(),
        node_id: "face:1".into(),
        labels: vec!["FaceInstance".into()],
        text: "face instance detected".into(),
        stimuli: Vec::new(),
        edges: Vec::new(),
        neighbors: vec!["TextObservation text: Anna".into()],
    }];
    let identity = GraphFaceIdentityLabel {
        identity_id: "identity:person:anna".into(),
        name: "Anna".into(),
    };

    Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .attach_face_identity(&cluster, "gpt-oss", &items, Some(&identity))
        .await
        .unwrap();

    constraint.assert_async().await;
    update.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_attaches_voice_identity() {
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
                .body_contains("VoiceIdentityRun")
                .body_contains("Identity")
                .body_contains("Person")
                .body_contains("HAS_VOICE_IDENTITY_RUN")
                .body_contains("HAS_IDENTITY")
                .body_contains("IDENTITY_OF")
                .body_contains("USED_CONTEXT")
                .body_contains("Anna")
                .body_contains("qdrant:voices:point-1");
            then.status(200).body(r#"{"results":[{}],"errors":[]}"#);
        })
        .await;

    let cluster = VectorCluster {
        cluster_id: "cluster:voice:1".into(),
        collection: "voices".into(),
        threshold: 0.9,
        centroid: vec![0.95, 0.05],
        mean_similarity: 0.95,
        members: vec![VectorClusterMember {
            point_id: "point-1".into(),
            average_similarity: 0.95,
        }],
    };
    let items = vec![GraphClusterItem {
        vector_id: "qdrant:voices:point-1".into(),
        node_id: "voice-signature:speaker:1".into(),
        labels: vec!["VoiceSignature".into()],
        text: "voice signature: f0 150 Hz, speech rate 4.5".into(),
        stimuli: vec!["audio: Anna said hello".into()],
        edges: Vec::new(),
        neighbors: Vec::new(),
    }];
    let identity = GraphVoiceIdentityLabel {
        identity_id: "identity:person:anna".into(),
        name: "Anna".into(),
    };

    Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .attach_voice_identity(&cluster, "gpt-oss", &items, Some(&identity))
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
                .body_contains("MATCH (anchor:GraphNode:Sensation)")
                .body_contains("combobulation_summary")
                .body_contains("coalesce(anchor.how")
                .body_contains("INCLUDED_IN_COMBOBULATION")
                .body_contains("ORDER BY datetime(anchor_at) ASC, anchor.id ASC")
                .body_contains("duration({seconds: $seconds})")
                .body_contains("coalesce(n.occurred_at")
                .body_contains("coalesce(n.how")
                .body_contains("datetime(occurred_at) >= datetime(anchor_at)")
                .body_contains("datetime(occurred_at) <= datetime(anchor_at) + duration({seconds: $seconds})")
                .body_contains("NOT (n)-[:INCLUDED_IN_COMBOBULATION]")
                .body_contains("timeline_order")
                .body_contains("ORDER BY datetime(occurred_at) ASC, timeline_order ASC, n.id ASC")
                .body_contains("RETURN anchor.id, anchor_at, row.id, row.event_id, row.labels, row.text, row.occurred_at")
                .body_contains("\"seconds\":30")
                .body_contains("\"limit\":80");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": ["anchor.id", "anchor_at", "n.id", "event_id", "labels(n)", "text", "occurred_at"],
                    "data": [
                        {"row": [
                            "sensation:audio:2",
                            "2026-05-05T12:35:00Z",
                            "sensation:audio:1",
                            "audio:1",
                            ["GraphNode", "Sensation"],
                            "audio sensation; transcript: hello",
                            "2026-05-05T12:34:56Z"
                        ]},
                        {"row": [
                            "sensation:audio:2",
                            "2026-05-05T12:35:00Z",
                            "sensation:audio:2",
                            "audio:1",
                            ["GraphNode", "Sensation"],
                            "audio sensation; transcript: there",
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

    assert_eq!(window.anchor_id, "sensation:audio:2");
    assert_eq!(window.anchor_at, "2026-05-05T12:35:00Z");
    assert_eq!(window.items.len(), 2);
    assert_eq!(window.items[0].id, "sensation:audio:1");
    assert_eq!(window.items[0].event_id, "audio:1");
    assert_eq!(window.items[0].labels, ["GraphNode", "Sensation"]);
    assert_eq!(window.items[1].text, "audio sensation; transcript: there");
    query.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_loads_previous_timeline_window_for_combobulation() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MATCH (anchor:GraphNode:Sensation)")
                .body_contains("datetime(anchor_at) < datetime($before)")
                .body_contains("INCLUDED_IN_COMBOBULATION")
                .body_contains("duration({seconds: $seconds})")
                .body_contains("coalesce(n.how")
                .body_contains("ORDER BY datetime(anchor_at) ASC, anchor.id ASC")
                .body_contains("ORDER BY datetime(occurred_at) ASC, timeline_order ASC, n.id ASC")
                .body_contains("RETURN anchor.id, anchor_at, row.id, row.event_id, row.labels, row.text, row.occurred_at")
                .body_contains("\"before\":\"2026-05-05T12:34:56Z\"")
                .body_contains("\"seconds\":30")
                .body_contains("\"limit\":80");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": ["anchor.id", "anchor_at", "n.id", "event_id", "labels(n)", "text", "occurred_at"],
                    "data": [
                        {"row": [
                            "sensation:audio:0",
                            "2026-05-05T12:34:30Z",
                            "sensation:audio:-1",
                            "audio:-1",
                            ["GraphNode", "Sensation"],
                            "audio sensation; transcript: earlier",
                            "2026-05-05T12:34:20Z"
                        ]},
                        {"row": [
                            "sensation:audio:0",
                            "2026-05-05T12:34:30Z",
                            "sensation:audio:0",
                            "audio:0",
                            ["GraphNode", "Sensation"],
                            "audio sensation; transcript: before",
                            "2026-05-05T12:34:30Z"
                        ]}
                    ]
                }],
                "errors": []
            }));
        })
        .await;

    let window = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .previous_timeline_window_for_combobulation(30, 80, "2026-05-05T12:34:56Z")
        .await
        .unwrap()
        .unwrap();

    assert_eq!(window.anchor_id, "sensation:audio:0");
    assert_eq!(window.anchor_at, "2026-05-05T12:34:30Z");
    assert_eq!(window.items.len(), 2);
    assert_eq!(window.items[0].id, "sensation:audio:-1");
    assert_eq!(window.items[1].event_id, "audio:0");
    assert_eq!(window.items[1].text, "audio sensation; transcript: before");
    query.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_loads_sensation_timeline_with_sensation_order() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MATCH (n:GraphNode:Sensation)")
                .body_contains("coalesce(n.how")
                .body_contains("timeline_order")
                .body_contains("ORDER BY datetime(occurred_at) DESC, timeline_order DESC, n.id DESC")
                .body_contains("RETURN row.id, row.labels, row.kind, row.text, row.occurred_at, row.formed_at")
                .body_contains("\"limit\":20");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": ["row.id", "row.labels", "row.kind", "row.text", "row.occurred_at", "row.formed_at"],
                    "data": [
                        {"row": [
                            "sensation:image:1",
                            ["GraphNode", "Sensation"],
                            "image",
                            psyche::IMAGE_SENSATION_TEXT,
                            "2026-05-05T12:34:56Z",
                            "2026-05-05T12:34:56Z"
                        ]},
                        {"row": [
                            "sensation:face_recognition:1",
                            ["GraphNode", "Sensation"],
                            "face_recognition",
                            psyche::face_count_sensation_text(1),
                            "2026-05-05T12:34:56Z",
                            "2026-05-05T12:34:57Z"
                        ]},
                        {"row": [
                            "sensation:face_identity:1",
                            ["GraphNode", "Sensation"],
                            "face_identity",
                            psyche::face_familiarity_sensation_text(false),
                            "2026-05-05T12:34:56Z",
                            "2026-05-05T12:34:57Z"
                        ]}
                    ]
                }],
                "errors": []
            }));
        })
        .await;

    let end = chrono::DateTime::parse_from_rfc3339("2026-05-05T12:35:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let items = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .sensation_timeline(None, end, 20)
        .await
        .unwrap();

    assert_eq!(items.len(), 3);
    assert_eq!(items[0].text, psyche::IMAGE_SENSATION_TEXT);
    assert_eq!(items[1].text, psyche::face_count_sensation_text(1));
    assert_eq!(
        items[2].text,
        psyche::face_familiarity_sensation_text(false)
    );
    query.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_loads_revisitable_timeline_window_for_combobulation() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MATCH (run:GraphNode:CombobulationRun)")
                .body_contains("run.source_texts")
                .body_contains("current_source_texts")
                .body_contains("size(run.source_ids) <= $limit")
                .body_contains("NOT EXISTS")
                .body_contains("coalesce(n.how")
                .body_contains("RETURN run.anchor_id, run.anchor_at, item.id, item.event_id, item.labels, item.text, item.occurred_at")
                .body_contains("\"limit\":80");
            then.status(200).json_body(json!({
                "results": [{
                    "columns": ["run.anchor_id", "run.anchor_at", "item.id", "item.event_id", "item.labels", "item.text", "item.occurred_at"],
                    "data": [
                        {"row": [
                            "sensation:audio:2",
                            "2026-05-05T12:35:00Z",
                            "sensation:audio:1",
                            "audio:1",
                            ["GraphNode", "Sensation"],
                            "audio sensation; transcript: hello",
                            "2026-05-05T12:34:56Z"
                        ]},
                        {"row": [
                            "sensation:audio:2",
                            "2026-05-05T12:35:00Z",
                            "sensation:audio:2",
                            "audio:2",
                            ["GraphNode", "Sensation"],
                            "audio sensation; transcript: there",
                            "2026-05-05T12:35:00Z"
                        ]}
                    ]
                }],
                "errors": []
            }));
        })
        .await;

    let window = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .latest_revisitable_timeline_window_for_combobulation(80)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(window.anchor_id, "sensation:audio:2");
    assert_eq!(window.anchor_at, "2026-05-05T12:35:00Z");
    assert_eq!(window.items.len(), 2);
    assert_eq!(window.items[0].id, "sensation:audio:1");
    assert_eq!(window.items[1].event_id, "audio:2");
    assert_eq!(window.items[1].text, "audio sensation; transcript: there");
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
                .body_contains("\"kind\":\"combobulation_summary\"")
                .body_contains("\"derived\":true")
                .body_contains("\"occurred_at\":\"2026-05-05T12:34:56Z\"")
                .body_contains("\"how\":\"I hear someone greeting me.\"")
                .body_contains("\"combobulation_run_id\"")
                .body_contains("\"awareness_id\":\"awareness:speech:2\"")
                .body_contains("\"model\":\"wit-test\"")
                .body_contains("\"embedding_model\":\"embed-test\"")
                .body_contains("\"source_count\":2")
                .body_contains("\"source_event_ids\":[\"audio:1\",\"audio:1\"]")
                .body_contains("\"source_started_at\":\"2026-05-05T12:34:56Z\"")
                .body_contains("\"source_ended_at\":\"2026-05-05T12:35:00Z\"")
                .body_contains("\"source_texts\":[\"speech: hello\",\"speech: there\"]");
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
                        event_id: "audio:1".into(),
                        labels: vec!["GraphNode".into(), "SpeechSegment".into()],
                        text: "speech: hello".into(),
                        occurred_at: "2026-05-05T12:34:56Z".into(),
                    },
                    GraphTimelineItem {
                        id: "speech:2".into(),
                        event_id: "audio:1".into(),
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
                emoji: None,
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
async fn neo4j_client_loads_latest_combobulation_emotion() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("CombobulationSummary");
            then.status(200).body(
                r#"{"results":[{"data":[{"row":["awareness:1","","I think someone is nearby. 🙂"]}]}],"errors":[]}"#,
            );
        })
        .await;

    let emotion = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .latest_combobulation_emotion()
        .await
        .unwrap()
        .unwrap();

    assert_eq!(emotion.id, "awareness:1");
    assert_eq!(emotion.emoji, "🙂");
    query.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_loads_latest_combobulation() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("RETURN n.id, text, emoji, formed_at");
            then.status(200).body(
                r#"{"results":[{"data":[{"row":["awareness:1","I think someone is nearby. 🙂","","2026-05-07T12:00:00Z"]}]}],"errors":[]}"#,
            );
        })
        .await;

    let latest = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .latest_combobulation()
        .await
        .unwrap()
        .unwrap();

    assert_eq!(latest.id, "awareness:1");
    assert_eq!(latest.text, "I think someone is nearby. 🙂");
    assert_eq!(latest.emoji.as_deref(), Some("🙂"));
    assert_eq!(latest.formed_at, "2026-05-07T12:00:00Z");
    query.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_loads_latest_combobulation_sensation_at() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("MATCH (n:GraphNode:Sensation)")
                .body_contains("combobulation_summary")
                .body_contains("RETURN occurred_at")
                .body_contains("ORDER BY datetime(occurred_at) DESC");
            then.status(200)
                .body(r#"{"results":[{"data":[{"row":["2026-05-07T12:00:00Z"]}]}],"errors":[]}"#);
        })
        .await;

    let occurred_at = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .latest_combobulation_sensation_at()
        .await
        .unwrap()
        .unwrap();

    assert_eq!(occurred_at, "2026-05-07T12:00:00Z");
    query.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_loads_latest_presentable_face_emotion() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("I turn my face into");
            then.status(200).body(
                r#"{"results":[{"data":[{"row":["impression:1","😐","I turn my face into a 😐."]}]}],"errors":[]}"#,
            );
        })
        .await;

    let emotion = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .latest_presentable_face_emotion()
        .await
        .unwrap()
        .unwrap();

    assert_eq!(emotion.id, "impression:1");
    assert_eq!(emotion.emoji, "😐");
    query.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_loads_latest_pending_speech_intention() {
    let server = MockServer::start_async().await;
    let query = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/db/neo4j/tx/commit")
                .body_contains("I ought to say: ")
                .body_contains("I start saying: ")
                .body_contains("I queue saying: ")
                .body_contains(r#"I start saying \\\"\" + words + \"\\\".\""#)
                .body_contains("RETURN n.id, words, formed_at");
            then.status(200).body(
                r#"{"results":[{"data":[{"row":["impression:1","Hello there.","2026-05-07T12:00:00Z"]}]}],"errors":[]}"#,
            );
        })
        .await;

    let intention = Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .latest_pending_speech_intention()
        .await
        .unwrap()
        .unwrap();

    assert_eq!(intention.id, "impression:1");
    assert_eq!(intention.text, "Hello there.");
    assert_eq!(intention.formed_at, "2026-05-07T12:00:00Z");
    query.assert_async().await;
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
                .body_contains("\"kind\":\"voice_identity\"")
                .body_contains("\"how\":\"I don't think I recognize this voice.\"")
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
                recognition: None,
            },
        )
        .await
        .unwrap();

    constraint.assert_async().await;
    update.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_attaches_known_voice_identity_sensation() {
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
                .body_contains("\"kind\":\"voice_identity\"")
                .body_contains("\"how\":\"I recognize Anna's voice.\"")
                .body_contains("\"matched_voice_id\":\"cluster:voice:1\"")
                .body_contains("\"identity_name\":\"Anna\"")
                .body_contains("\"nearest_voice_vector_id\":\"known-point\"")
                .body_contains("MATCHED_VOICE")
                .body_contains("RECOGNIZED_AS")
                .body_contains("MATCHED_NEAREST_VECTOR");
            then.status(200).body(r#"{"results":[{}],"errors":[]}"#);
        })
        .await;

    Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .attach_voice_recognition(
            &test_voice_clip(),
            "voxudio",
            &test_voice_recognition(Some(psyche::GraphVoiceMatch {
                voice_id: "cluster:voice:1".into(),
                identity: Some("Anna".into()),
                nearest_vector_id: "known-point".into(),
                score: 0.93,
            })),
        )
        .await
        .unwrap();

    constraint.assert_async().await;
    update.assert_async().await;
}

#[tokio::test]
async fn neo4j_client_attaches_unknown_voice_match_sensation() {
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
                .body_contains("\"kind\":\"voice_identity\"")
                .body_contains("\"how\":\"I recognize the voice, but I don't know who it is.\"")
                .body_contains("\"matched_voice_id\":\"cluster:voice:2\"")
                .body_contains("\"nearest_voice_vector_id\":\"known-point-2\"")
                .body_contains("MATCHED_VOICE");
            then.status(200).body(r#"{"results":[{}],"errors":[]}"#);
        })
        .await;

    Neo4jClient::new(server.base_url(), "neo4j".into(), "password".into())
        .attach_voice_recognition(
            &test_voice_clip(),
            "voxudio",
            &test_voice_recognition(Some(psyche::GraphVoiceMatch {
                voice_id: "cluster:voice:2".into(),
                identity: None,
                nearest_vector_id: "known-point-2".into(),
                score: 0.91,
            })),
        )
        .await
        .unwrap();

    constraint.assert_async().await;
    update.assert_async().await;
}

fn test_voice_clip() -> GraphVoiceClip {
    GraphVoiceClip {
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
    }
}

fn test_voice_recognition(recognition: Option<psyche::GraphVoiceMatch>) -> GraphVoiceRecognition {
    GraphVoiceRecognition {
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
        recognition,
    }
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

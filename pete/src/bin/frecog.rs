use std::{sync::Arc, time::Duration};

use anyhow::Context;
use clap::Parser;
use dotenvy::dotenv;
use pete::{EventBus, init_logging};
use psyche::{
    FaceDetector, FaceIdDetector, GraphFaceDetection, GraphImageFrame, Neo4jClient, QdrantClient,
    image_content_id,
};
use tokio::time::{MissedTickBehavior, interval};
use tracing::{debug, error, info};

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Recognize faces in stored Image graph nodes and link the results"
)]
struct Cli {
    /// Neo4j bolt or HTTP URI.
    #[arg(long, env = "NEO4J_URI", default_value = "bolt://localhost:7687")]
    neo4j_uri: String,
    /// Neo4j username.
    #[arg(long, env = "NEO4J_USER", default_value = "neo4j")]
    neo4j_user: String,
    /// Neo4j password.
    #[arg(long, env = "NEO4J_PASS", default_value = "password")]
    neo4j_pass: String,
    /// Qdrant HTTP endpoint.
    #[arg(long, env = "QDRANT_URL", default_value = "http://localhost:6333")]
    qdrant_url: String,
    /// Delay between graph polling attempts.
    #[arg(long, env = "FRECOG_POLL_MS", default_value_t = 1000)]
    poll_ms: u64,
    /// Detector label stored on graph vector nodes and face-recognition runs.
    #[arg(long, env = "FRECOG_DETECTOR", default_value = "face_id")]
    detector: String,
    /// Process at most one frame and exit.
    #[arg(long)]
    once: bool,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let (bus, _user_rx) = EventBus::new();
    init_logging(bus.log_sender());
    dotenv().ok();

    let cli = Cli::parse();
    let graph = Neo4jClient::new(cli.neo4j_uri, cli.neo4j_user, cli.neo4j_pass);
    let qdrant = QdrantClient::new(cli.qdrant_url);
    let detector = Arc::new(
        FaceIdDetector::from_hf()
            .await
            .context("failed to initialize face recognition detector")?,
    );

    if cli.once {
        process_next_frame(&graph, &qdrant, detector, &cli.detector).await?;
        return Ok(());
    }

    let mut ticker = interval(Duration::from_millis(cli.poll_ms.max(100)));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    info!("face recognition loop started");
    loop {
        ticker.tick().await;
        if let Err(err) = process_next_frame(&graph, &qdrant, detector.clone(), &cli.detector).await
        {
            error!(error = %err, "face recognition loop iteration failed");
        }
    }
}

async fn process_next_frame(
    graph: &Neo4jClient,
    qdrant: &QdrantClient,
    detector: Arc<dyn FaceDetector>,
    detector_name: &str,
) -> anyhow::Result<()> {
    let Some(frame) = graph
        .latest_unprocessed_image_frame_for_face_recognition()
        .await
        .context("failed to load latest unprocessed image frame")?
    else {
        debug!("no unprocessed image frames found");
        return Ok(());
    };

    info!(image_id = %frame.id, "recognizing faces in image frame");
    let faces = detector
        .detect_faces(&frame.image)
        .await
        .with_context(|| format!("failed to recognize faces in image {}", frame.id))?;
    let mut detections = Vec::with_capacity(faces.len());
    for (index, (mut crop, embedding)) in faces.into_iter().enumerate() {
        if crop.captured_at.is_none() {
            crop.captured_at = frame
                .image
                .captured_at
                .clone()
                .or_else(|| frame.occurred_at.clone());
        }
        let face_id = image_content_id(&crop);
        let vector_id = qdrant
            .store_face_vector_for_sensation(
                Some(&face_id),
                Some(&frame.id),
                frame.sensation_id.as_deref(),
                &embedding,
            )
            .await
            .with_context(|| format!("failed to store face vector for {face_id}"))?
            .to_string();
        detections.push(GraphFaceDetection {
            index,
            face_id,
            crop,
            vector_id,
            embedding_len: embedding.len(),
        });
    }

    graph
        .attach_face_recognition(&frame, detector_name, &detections)
        .await
        .with_context(|| format!("failed to attach face recognition for image {}", frame.id))?;
    log_completion(&frame, detections.len());
    Ok(())
}

fn log_completion(frame: &GraphImageFrame, face_count: usize) {
    info!(
        image_id = %frame.id,
        sensation_id = frame.sensation_id.as_deref().unwrap_or(""),
        face_count,
        "attached face recognition"
    );
}

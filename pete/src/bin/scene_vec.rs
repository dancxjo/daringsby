use std::{path::PathBuf, time::Duration};

use anyhow::{Context, anyhow, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use clap::Parser;
use dotenvy::dotenv;
use open_clip_inference::VisionEmbedder;
use pete::{EventBus, init_logging};
use psyche::{GraphImageFrame, GraphSceneVectorization, Neo4jClient, QdrantClient};
use tokio::time::{MissedTickBehavior, interval};
use tracing::{debug, error, info};

const DEFAULT_SCENE_VEC_MODEL: &str = "RuteNL/MobileCLIP2-S3-OpenCLIP-ONNX";

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Vectorize stored Image graph nodes with CLIP and link scene vectors"
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
    /// Hugging Face CLIP ONNX model id.
    #[arg(long, env = "SCENE_VEC_MODEL", default_value = DEFAULT_SCENE_VEC_MODEL)]
    model: String,
    /// Local converted OpenCLIP model directory. Overrides --model.
    #[arg(long, env = "SCENE_VEC_MODEL_DIR")]
    model_dir: Option<PathBuf>,
    /// Delay between graph polling attempts.
    #[arg(long, env = "SCENE_VEC_POLL_MS", default_value_t = 1000)]
    poll_ms: u64,
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
    let vectorizer = SceneVectorizer::new(cli.model, cli.model_dir).await?;

    if cli.once {
        process_next_frame(&graph, &qdrant, &vectorizer).await?;
        return Ok(());
    }

    let mut ticker = interval(Duration::from_millis(cli.poll_ms.max(100)));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    info!("scene vectorization loop started");
    loop {
        ticker.tick().await;
        if let Err(err) = process_next_frame(&graph, &qdrant, &vectorizer).await {
            error!(error = %err, "scene vectorization loop iteration failed");
        }
    }
}

async fn process_next_frame(
    graph: &Neo4jClient,
    qdrant: &QdrantClient,
    vectorizer: &SceneVectorizer,
) -> anyhow::Result<()> {
    let Some(frame) = graph
        .latest_unprocessed_image_frame_for_scene_vectorization()
        .await
        .context("failed to load latest unprocessed image frame")?
    else {
        debug!("no unprocessed image frames found");
        return Ok(());
    };

    info!(image_id = %frame.id, "vectorizing image scene");
    let scene = vectorizer
        .vectorize(&frame, qdrant)
        .await
        .with_context(|| format!("failed to vectorize scene for image {}", frame.id))?;
    graph
        .attach_scene_vectorization(&frame, &vectorizer.model_label, &scene)
        .await
        .with_context(|| {
            format!(
                "failed to attach scene vectorization for image {}",
                frame.id
            )
        })?;
    info!(
        image_id = %frame.id,
        vector_id = %scene.vector_id,
        embedding_len = scene.embedding_len,
        "attached scene vectorization"
    );
    Ok(())
}

struct SceneVectorizer {
    embedder: VisionEmbedder,
    model_label: String,
}

impl SceneVectorizer {
    async fn new(model: String, model_dir: Option<PathBuf>) -> anyhow::Result<Self> {
        if let Some(model_dir) = model_dir {
            let embedder = VisionEmbedder::from_local_dir(&model_dir)
                .build()
                .with_context(|| {
                    format!(
                        "failed to load local CLIP vision model from {}",
                        model_dir.display()
                    )
                })?;
            let model_label = model_dir.display().to_string();
            info!(model = %model_label, "local CLIP vision model loaded");
            return Ok(Self {
                embedder,
                model_label,
            });
        }

        let embedder = VisionEmbedder::from_hf(&model)
            .build()
            .await
            .with_context(|| format!("failed to load CLIP vision model {model}"))?;
        info!(model = %model, "CLIP vision model loaded");
        Ok(Self {
            embedder,
            model_label: model,
        })
    }

    async fn vectorize(
        &self,
        frame: &GraphImageFrame,
        qdrant: &QdrantClient,
    ) -> anyhow::Result<GraphSceneVectorization> {
        let image = decode_image_frame(frame)?;
        let embedding = self
            .embedder
            .embed_image(&image)
            .context("failed to run CLIP image embedding")?
            .to_vec();
        if embedding.is_empty() {
            bail!("CLIP model returned no embedding for image {}", frame.id);
        }
        let vector_id = qdrant
            .store_scene_vector_for_sensation(
                &frame.id,
                frame.sensation_id.as_deref(),
                &self.model_label,
                &embedding,
            )
            .await
            .context("failed to store scene vector")?
            .to_string();
        Ok(GraphSceneVectorization {
            vector_id,
            embedding_len: embedding.len(),
        })
    }
}

fn decode_image_frame(frame: &GraphImageFrame) -> anyhow::Result<image::DynamicImage> {
    if frame.image.base64.trim().is_empty() {
        bail!("image {} had no payload", frame.id);
    }
    if !frame.image.mime.to_ascii_lowercase().starts_with("image/") {
        bail!("unsupported image MIME type {}", frame.image.mime);
    }
    let bytes = BASE64_STANDARD
        .decode(frame.image.base64.trim().as_bytes())
        .with_context(|| format!("failed to decode image {} payload", frame.id))?;
    image::load_from_memory(&bytes)
        .map_err(|err| anyhow!(err))
        .with_context(|| format!("failed to decode image {}", frame.id))
}

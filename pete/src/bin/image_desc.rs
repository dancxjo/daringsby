use std::time::Duration;

use anyhow::{Context, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use clap::Parser;
use dotenvy::dotenv;
use lingproc::{ImageData as LImageData, LlmInstruction, Vectorizer};
use pete::{EventBus, init_logging, ollama_provider_from_args};
use psyche::{
    Doer, GraphImageDescription, GraphImageFrame, GraphLatestCombobulation, IMAGE_CAPTION_PROMPT,
    Neo4jClient, QdrantClient, with_default_system_prompt,
};
use tokio::time::{MissedTickBehavior, interval};
use tracing::{error, info, trace, warn};

const DEFAULT_IMAGE_DESCRIPTION_MODEL: &str = "gemma4";

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Describe stored Image graph nodes with an LLM and embed the descriptions"
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
    /// URL of the image-description Ollama server.
    #[arg(
        long,
        env = "IMAGE_DESCRIPTION_HOST",
        default_value = "http://localhost:11434"
    )]
    image_description_host: String,
    /// Vision-capable model name to use for image descriptions.
    #[arg(
        long,
        env = "IMAGE_DESCRIPTION_MODEL",
        default_value = DEFAULT_IMAGE_DESCRIPTION_MODEL
    )]
    image_description_model: String,
    /// URL of the embeddings Ollama server.
    #[arg(
        long,
        env = "EMBEDDINGS_HOST",
        default_value = "http://localhost:11434"
    )]
    embeddings_host: String,
    /// Model name to use for description text embeddings.
    #[arg(long, env = "EMBEDDINGS_MODEL", default_value = "embeddinggemma")]
    embeddings_model: String,
    /// Delay between graph polling attempts.
    #[arg(long, env = "IMAGE_DESCRIPTION_POLL_MS", default_value_t = 1000)]
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
    ensure_vision_model(&cli.image_description_model)?;
    let describer =
        ollama_provider_from_args(&cli.image_description_host, &cli.image_description_model)?;
    let vectorizer = ollama_provider_from_args(&cli.embeddings_host, &cli.embeddings_model)?;
    let processor = ImageDescriptionProcessor {
        describer,
        vectorizer,
        vision_model: cli.image_description_model,
        embedding_model: cli.embeddings_model,
    };

    if cli.once {
        process_next_frame(&graph, &qdrant, &processor).await?;
        return Ok(());
    }

    let mut ticker = interval(Duration::from_millis(cli.poll_ms.max(100)));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    info!("image description loop started");
    loop {
        ticker.tick().await;
        if let Err(err) = process_next_frame(&graph, &qdrant, &processor).await {
            error!(error = %err, "image description loop iteration failed");
        }
    }
}

async fn process_next_frame(
    graph: &Neo4jClient,
    qdrant: &QdrantClient,
    processor: &ImageDescriptionProcessor,
) -> anyhow::Result<()> {
    let Some(frame) = graph
        .latest_unprocessed_image_frame_for_description()
        .await
        .context("failed to load latest unprocessed image frame")?
    else {
        trace!("no undescribed image frames found");
        return Ok(());
    };

    let combobulation = graph.latest_combobulation().await.unwrap_or(None);

    info!(image_id = %frame.id, "describing image frame");
    let description = processor
        .describe(&frame, qdrant, combobulation.as_ref())
        .await
        .with_context(|| format!("failed to describe image {}", frame.id))?;
    graph
        .attach_image_description(
            &frame,
            &processor.vision_model,
            &processor.embedding_model,
            &description,
        )
        .await
        .with_context(|| format!("failed to attach image description for image {}", frame.id))?;
    info!(
        image_id = %frame.id,
        description_id = %description.description_id,
        vector_id = %description.vector_id,
        embedding_len = description.embedding_len,
        "attached image description"
    );
    Ok(())
}

struct ImageDescriptionProcessor {
    describer: lingproc::OllamaProvider,
    vectorizer: lingproc::OllamaProvider,
    vision_model: String,
    embedding_model: String,
}

impl ImageDescriptionProcessor {
    async fn describe(
        &self,
        frame: &GraphImageFrame,
        qdrant: &QdrantClient,
        combobulation: Option<&GraphLatestCombobulation>,
    ) -> anyhow::Result<GraphImageDescription> {
        if !frame.image.mime.to_ascii_lowercase().starts_with("image/") {
            bail!("unsupported image MIME type {}", frame.image.mime);
        }

        let text = if frame.image.base64.trim().is_empty() {
            "I can't see anything.".to_string()
        } else {
            let (base64, image_bytes) = normalized_image_base64(&frame.image.base64)
                .with_context(|| format!("invalid base64 image payload for {}", frame.id))?;
            trace!(
                image_id = %frame.id,
                image_base64_len = base64.len(),
                image_bytes,
                "including image payload in Ollama request"
            );
            let command = if let Some(comb) = combobulation {
                let context = format!(
                    "\n\nThe current situation you understand is:\n{}",
                    comb.text.trim()
                );
                with_default_system_prompt(format!("{IMAGE_CAPTION_PROMPT}{context}"))
            } else {
                with_default_system_prompt(IMAGE_CAPTION_PROMPT)
            };
            self.describer
                .follow(LlmInstruction {
                    command,
                    images: vec![LImageData {
                        mime: frame.image.mime.clone(),
                        base64,
                        captured_at: frame.image.captured_at.clone(),
                    }],
                })
                .await?
                .trim()
                .to_string()
        };
        let embedding = self
            .vectorizer
            .vectorize(&text)
            .await
            .context("failed to embed image description")?;
        if embedding.is_empty() {
            bail!("embedding model returned no vector for image {}", frame.id);
        }

        let description_id = image_description_id(&frame.id);
        let mut related = vec![frame.id.as_str()];
        if let Some(sensation_id) = &frame.sensation_id {
            related.push(sensation_id.as_str());
        }
        let vector_id = qdrant
            .store_image_description_vector_for_node_with_model(
                &frame.id,
                &text,
                &description_id,
                &related,
                Some(&self.embedding_model),
                &embedding,
            )
            .await
            .context("failed to store image description vector")?
            .to_string();

        Ok(GraphImageDescription {
            description_id,
            text,
            vector_id,
            embedding_len: embedding.len(),
        })
    }
}

fn image_description_id(image_id: &str) -> String {
    format!("image-description-text:{image_id}")
}

fn normalized_image_base64(value: &str) -> anyhow::Result<(String, usize)> {
    let base64 = value
        .split_once(',')
        .map_or_else(|| value.trim(), |(_, encoded)| encoded.trim())
        .to_string();
    let bytes = BASE64_STANDARD
        .decode(base64.as_bytes())
        .context("failed to decode base64 image")?;
    Ok((base64, bytes.len()))
}

fn ensure_vision_model(model: &str) -> anyhow::Result<()> {
    let normalized = model.to_ascii_lowercase();
    if normalized == "gpt-oss" || normalized.starts_with("gpt-oss:") {
        bail!(
            "IMAGE_DESCRIPTION_MODEL={model} is text-only in Ollama; use a vision-capable model like {DEFAULT_IMAGE_DESCRIPTION_MODEL}"
        );
    }
    if matches!(
        normalized.as_str(),
        "gemma3:270m" | "gemma3:1b" | "gemma4:270m" | "gemma4:1b"
    ) {
        bail!(
            "IMAGE_DESCRIPTION_MODEL={model} is text-only in Ollama; use a vision-capable model like {DEFAULT_IMAGE_DESCRIPTION_MODEL}"
        );
    }
    if normalized.contains("llama3") || normalized.contains("qwen3") {
        warn!(
            %model,
            "image description model does not look vision-capable; Ollama may ignore attached images"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_image_base64_strips_data_url_prefix() {
        let (base64, bytes) = normalized_image_base64("data:image/jpeg;base64,aGVsbG8=").unwrap();

        assert_eq!(base64, "aGVsbG8=");
        assert_eq!(bytes, 5);
    }

    #[test]
    fn ensure_vision_model_rejects_gpt_oss() {
        let err = ensure_vision_model("gpt-oss").unwrap_err();

        assert!(err.to_string().contains("text-only"));
    }

    #[test]
    fn ensure_vision_model_allows_default_gemma4() {
        ensure_vision_model(DEFAULT_IMAGE_DESCRIPTION_MODEL).unwrap();
    }

    #[test]
    fn ensure_vision_model_rejects_text_only_gemma_variants() {
        for model in ["gemma3:270m", "gemma3:1b", "gemma4:270m", "gemma4:1b"] {
            let err = ensure_vision_model(model).unwrap_err();

            assert!(err.to_string().contains("text-only"));
            assert!(err.to_string().contains(DEFAULT_IMAGE_DESCRIPTION_MODEL));
        }
    }
}

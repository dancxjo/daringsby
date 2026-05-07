use anyhow::Context;
use clap::Parser;
use dotenvy::dotenv;
use pete::{EventBus, init_logging};
use psyche::{Neo4jClient, QdrantClient, qdrant_vector_collections};
use tracing::{info, warn};

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Prune Pete's stores down to raw sensations, audio clips, and image frames"
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
    /// Number of Neo4j nodes to delete per batch.
    #[arg(long, env = "RAW_RETENTION_BATCH_SIZE", default_value_t = 500)]
    batch_size: usize,
    /// Print what would be removed without deleting anything.
    #[arg(long)]
    dry_run: bool,
    /// Confirm destructive deletion.
    #[arg(long)]
    confirm: bool,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let (bus, _user_rx) = EventBus::new();
    init_logging(bus.log_sender());
    dotenv().ok();

    let cli = Cli::parse();
    if !cli.dry_run && !cli.confirm {
        anyhow::bail!("refusing to delete without --confirm; use --dry-run to inspect first");
    }

    let graph = Neo4jClient::new(cli.neo4j_uri, cli.neo4j_user, cli.neo4j_pass);
    let qdrant = QdrantClient::new(cli.qdrant_url);

    if cli.dry_run {
        dry_run(&graph, &qdrant).await?;
    } else {
        prune(&graph, &qdrant, cli.batch_size).await?;
    }

    Ok(())
}

async fn dry_run(graph: &Neo4jClient, qdrant: &QdrantClient) -> anyhow::Result<()> {
    let graph_count = graph
        .count_non_raw_graph_nodes()
        .await
        .context("failed to count non-raw graph nodes")?;
    let audio_transcript_count = graph
        .count_audio_clip_transcript_properties()
        .await
        .context("failed to count raw audio transcript properties")?;
    let collections = existing_qdrant_collections(qdrant).await?;

    println!("would delete {graph_count} non-raw Neo4j graph nodes");
    println!("would clear transcript fields from {audio_transcript_count} raw audio clips");
    if collections.is_empty() {
        println!("no known Qdrant vector collections found");
    } else {
        println!(
            "would delete Qdrant collections: {}",
            collections.join(", ")
        );
    }
    Ok(())
}

async fn prune(
    graph: &Neo4jClient,
    qdrant: &QdrantClient,
    batch_size: usize,
) -> anyhow::Result<()> {
    let deleted_graph_nodes = graph
        .detach_delete_non_raw_graph_nodes(batch_size)
        .await
        .context("failed to delete non-raw graph nodes")?;
    info!(deleted_graph_nodes, "deleted non-raw Neo4j graph nodes");
    let cleared_audio_transcripts = graph
        .clear_audio_clip_transcript_properties()
        .await
        .context("failed to clear raw audio transcript properties")?;
    info!(
        cleared_audio_transcripts,
        "cleared transcript fields from raw audio clips"
    );

    let mut deleted_collections = Vec::new();
    let mut missing_collections = Vec::new();
    for collection in qdrant_vector_collections() {
        let deleted = qdrant
            .delete_collection_if_exists(collection)
            .await
            .with_context(|| format!("failed to delete Qdrant collection {collection}"))?;
        if deleted {
            deleted_collections.push(*collection);
        } else {
            missing_collections.push(*collection);
        }
    }

    if !missing_collections.is_empty() {
        warn!(
            collections = ?missing_collections,
            "known Qdrant collections were already absent"
        );
    }

    println!("deleted {deleted_graph_nodes} non-raw Neo4j graph nodes");
    println!("cleared transcript fields from {cleared_audio_transcripts} raw audio clips");
    if deleted_collections.is_empty() {
        println!("deleted no Qdrant collections");
    } else {
        println!(
            "deleted Qdrant collections: {}",
            deleted_collections.join(", ")
        );
    }
    Ok(())
}

async fn existing_qdrant_collections(qdrant: &QdrantClient) -> anyhow::Result<Vec<&'static str>> {
    let mut collections = Vec::new();
    for collection in qdrant_vector_collections() {
        if qdrant
            .collection_exists(collection)
            .await
            .with_context(|| format!("failed to inspect Qdrant collection {collection}"))?
        {
            collections.push(*collection);
        }
    }
    Ok(collections)
}

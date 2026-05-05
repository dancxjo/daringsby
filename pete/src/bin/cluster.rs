use anyhow::Context;
use clap::Parser;
use dotenvy::dotenv;
use pete::{EventBus, init_logging};
use psyche::{Neo4jClient, QdrantClient, find_vector_clusters};
use tracing::info;

const ALGORITHM: &str = "cosine-threshold-components/v1";

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Find cosine-similarity clusters in a Qdrant collection and link them into Neo4j"
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
    /// Qdrant collection to cluster.
    #[arg(long, env = "CLUSTER_COLLECTION", default_value = "memories")]
    collection: String,
    /// Minimum cosine similarity for an edge between two vectors.
    #[arg(long, env = "CLUSTER_THRESHOLD", default_value_t = 0.86)]
    threshold: f32,
    /// Minimum number of points required to keep a cluster.
    #[arg(long, env = "CLUSTER_MIN_SIZE", default_value_t = 3)]
    min_size: usize,
    /// Maximum points to read from Qdrant in one discovery run.
    #[arg(long, env = "CLUSTER_MAX_POINTS", default_value_t = 1000)]
    max_points: usize,
    /// Qdrant scroll page size.
    #[arg(long, env = "CLUSTER_PAGE_SIZE", default_value_t = 256)]
    page_size: usize,
    /// Print results without writing cluster nodes to Neo4j.
    #[arg(long)]
    dry_run: bool,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let (bus, _user_rx) = EventBus::new();
    init_logging(bus.log_sender());
    dotenv().ok();

    let cli = Cli::parse();
    anyhow::ensure!(
        (0.0..=1.0).contains(&cli.threshold),
        "cluster threshold must be between 0.0 and 1.0"
    );

    let qdrant = QdrantClient::new(cli.qdrant_url);
    let points = qdrant
        .scroll_vectors(&cli.collection, cli.max_points, cli.page_size)
        .await
        .with_context(|| format!("failed to load vectors from {}", cli.collection))?;
    let clusters = find_vector_clusters(&cli.collection, &points, cli.threshold, cli.min_size);

    info!(
        collection = %cli.collection,
        point_count = points.len(),
        cluster_count = clusters.len(),
        threshold = cli.threshold,
        min_size = cli.min_size,
        "cluster discovery finished"
    );
    for cluster in &clusters {
        info!(
            cluster_id = %cluster.cluster_id,
            member_count = cluster.members.len(),
            mean_similarity = cluster.mean_similarity,
            "found vector cluster"
        );
    }

    if cli.dry_run {
        println!(
            "found {} clusters from {} {} points",
            clusters.len(),
            points.len(),
            cli.collection
        );
        for cluster in &clusters {
            println!(
                "{} members={} mean_similarity={:.3}",
                cluster.cluster_id,
                cluster.members.len(),
                cluster.mean_similarity
            );
        }
        return Ok(());
    }

    let graph = Neo4jClient::new(cli.neo4j_uri, cli.neo4j_user, cli.neo4j_pass);
    graph
        .attach_vector_clusters(
            &cli.collection,
            ALGORITHM,
            cli.threshold,
            cli.min_size,
            points.len(),
            &clusters,
        )
        .await
        .context("failed to attach vector clusters to Neo4j")?;
    Ok(())
}

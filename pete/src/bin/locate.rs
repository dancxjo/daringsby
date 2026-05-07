use std::time::Duration;

use anyhow::Context;
use clap::Parser;
use dotenvy::dotenv;
use pete::{EventBus, init_logging};
use psyche::{GraphGeolocation, Neo4jClient, QdrantClient, geoloc_vector};
use tokio::time::{MissedTickBehavior, interval};
use tracing::{error, info, trace};

const GEOLOCATION_MODEL: &str = "earth-unit-sphere/v1";

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Vectorize stored Geolocation graph nodes and link the results"
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
    #[arg(long, env = "LOCATE_POLL_MS", default_value_t = 1000)]
    poll_ms: u64,
    /// Process at most one geolocation and exit.
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

    if cli.once {
        process_next_geolocation(&graph, &qdrant).await?;
        return Ok(());
    }

    let mut ticker = interval(Duration::from_millis(cli.poll_ms.max(100)));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    info!("geolocation vectorization loop started");
    loop {
        ticker.tick().await;
        if let Err(err) = process_next_geolocation(&graph, &qdrant).await {
            error!(error = %err, "geolocation vectorization loop iteration failed");
        }
    }
}

async fn process_next_geolocation(
    graph: &Neo4jClient,
    qdrant: &QdrantClient,
) -> anyhow::Result<()> {
    let Some(geolocation) = graph
        .latest_unprocessed_geolocation_for_vectorization()
        .await
        .context("failed to load latest unprocessed geolocation")?
    else {
        trace!("no unprocessed geolocations found");
        return Ok(());
    };

    info!(geolocation_id = %geolocation.id, "vectorizing geolocation");
    let vector = geoloc_vector(&geolocation.loc);
    let vector_id = qdrant
        .store_geolocation_vector_for(
            &geolocation.id,
            geolocation.loc.latitude,
            geolocation.loc.longitude,
            &vector,
        )
        .await
        .with_context(|| format!("failed to store geolocation vector for {}", geolocation.id))?
        .to_string();
    graph
        .attach_geolocation_vectorization(&geolocation, GEOLOCATION_MODEL, &vector_id, vector.len())
        .await
        .with_context(|| {
            format!(
                "failed to attach geolocation vectorization for {}",
                geolocation.id
            )
        })?;
    log_completion(&geolocation, &vector_id);
    Ok(())
}

fn log_completion(geolocation: &GraphGeolocation, vector_id: &str) {
    info!(
        geolocation_id = %geolocation.id,
        sensation_id = geolocation.sensation_id.as_deref().unwrap_or(""),
        vector_id,
        "attached geolocation vectorization"
    );
}

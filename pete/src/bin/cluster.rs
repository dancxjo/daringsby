use std::time::Duration;

use anyhow::Context;
use clap::Parser;
use dotenvy::dotenv;
use lingproc::{Doer, LlmInstruction};
use pete::{EventBus, init_logging, ollama_provider_from_args};
use psyche::{
    GraphClusterItem, GraphFaceIdentityLabel, GraphVoiceIdentityLabel, Neo4jClient, QdrantClient,
    VectorCluster, find_vector_clusters, qdrant_vector_collections,
};
use tokio::time::{MissedTickBehavior, interval};
use tracing::{debug, error, info, warn};

const ALGORITHM: &str = "cosine-threshold-components/v1";
const FACE_COLLECTION: &str = "faces";
const VOICE_COLLECTION: &str = "voices";

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Continuously find vector clusters, link them into Neo4j, and identify face and voice clusters"
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
    /// Qdrant collection to cluster. Repeat or use commas; omitted means every known vector collection.
    #[arg(
        long = "collection",
        env = "CLUSTER_COLLECTION",
        value_name = "COLLECTION",
        value_delimiter = ','
    )]
    collection: Vec<String>,
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
    /// URL of the wits Ollama server.
    #[arg(long, env = "WITS_HOST", default_value = "http://localhost:11434")]
    wits_host: String,
    /// Model name to use for face and voice identity extraction.
    #[arg(long, env = "WITS_MODEL", default_value = "gpt-oss")]
    wits_model: String,
    /// Maximum graph items to present to the LLM for each cluster identity.
    #[arg(long, env = "CLUSTER_LABEL_ITEM_LIMIT", default_value_t = 24)]
    label_item_limit: usize,
    /// Delay between cluster discovery passes.
    #[arg(long, env = "CLUSTER_POLL_MS", default_value_t = 5000)]
    poll_ms: u64,
    /// Run one clustering pass and exit.
    #[arg(long)]
    once: bool,
    /// Print results without writing cluster nodes to Neo4j or identifying faces/voices.
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

    let qdrant = QdrantClient::new(cli.qdrant_url.clone());
    let graph = Neo4jClient::new(
        cli.neo4j_uri.clone(),
        cli.neo4j_user.clone(),
        cli.neo4j_pass.clone(),
    );
    let labeler = if cli.dry_run {
        None
    } else {
        Some(ClusterLabelProcessor {
            doer: ollama_provider_from_args(&cli.wits_host, &cli.wits_model)?,
            llm_model: cli.wits_model.clone(),
        })
    };

    if cli.once || cli.dry_run {
        run_cluster_pass(&cli, &qdrant, &graph, labeler.as_ref()).await?;
        return Ok(());
    }

    let mut ticker = interval(Duration::from_millis(cli.poll_ms.max(250)));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let collections = selected_collections(&cli.collection);
    info!(
        collections = ?collections,
        threshold = cli.threshold,
        min_size = cli.min_size,
        poll_ms = cli.poll_ms,
        "cluster discovery loop started"
    );
    loop {
        ticker.tick().await;
        if let Err(err) = run_cluster_pass(&cli, &qdrant, &graph, labeler.as_ref()).await {
            error!(
                error = %err,
                error_debug = ?err,
                "cluster discovery loop iteration failed"
            );
        }
    }
}

async fn run_cluster_pass(
    cli: &Cli,
    qdrant: &QdrantClient,
    graph: &Neo4jClient,
    labeler: Option<&ClusterLabelProcessor>,
) -> anyhow::Result<()> {
    let collections = selected_collections(&cli.collection);
    let skip_missing_collections = cli.collection.is_empty();

    for collection in collections {
        run_cluster_collection(
            cli,
            qdrant,
            graph,
            labeler,
            &collection,
            skip_missing_collections,
        )
        .await?;
    }

    Ok(())
}

async fn run_cluster_collection(
    cli: &Cli,
    qdrant: &QdrantClient,
    graph: &Neo4jClient,
    labeler: Option<&ClusterLabelProcessor>,
    collection: &str,
    skip_missing_collection: bool,
) -> anyhow::Result<()> {
    let points = qdrant
        .scroll_vectors_if_collection_exists(collection, cli.max_points, cli.page_size)
        .await
        .with_context(|| format!("failed to load vectors from {collection}"))?;
    let Some(points) = points else {
        if skip_missing_collection {
            debug!(collection, "skipping missing Qdrant collection");
            return Ok(());
        }
        anyhow::bail!("Qdrant collection {collection} does not exist");
    };
    let clusters = find_vector_clusters(collection, &points, cli.threshold, cli.min_size);

    info!(
        collection = %collection,
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
            collection
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

    graph
        .attach_vector_clusters(
            collection,
            ALGORITHM,
            cli.threshold,
            cli.min_size,
            points.len(),
            &clusters,
        )
        .await
        .context("failed to attach vector clusters to Neo4j")?;

    if let Some(labeler) = labeler {
        process_new_cluster_labels(cli, graph, labeler, &clusters).await?;
    }
    Ok(())
}

fn selected_collections(requested: &[String]) -> Vec<String> {
    let collections = if requested.is_empty() {
        qdrant_vector_collections()
            .iter()
            .map(|collection| (*collection).to_string())
            .collect::<Vec<_>>()
    } else {
        requested
            .iter()
            .flat_map(|value| value.split(','))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>()
    };

    let mut deduped = Vec::new();
    for collection in collections {
        if !deduped.contains(&collection) {
            deduped.push(collection);
        }
    }
    deduped
}

async fn process_new_cluster_labels(
    cli: &Cli,
    graph: &Neo4jClient,
    labeler: &ClusterLabelProcessor,
    clusters: &[VectorCluster],
) -> anyhow::Result<()> {
    for cluster in clusters {
        if cluster.collection == FACE_COLLECTION {
            identify_new_face_cluster(cli, graph, labeler, cluster).await?;
            continue;
        }
        if cluster.collection == VOICE_COLLECTION {
            identify_new_voice_cluster(cli, graph, labeler, cluster).await?;
            continue;
        }
        debug!(
            cluster_id = %cluster.cluster_id,
            collection = %cluster.collection,
            "skipping non-identity cluster label"
        );
    }
    Ok(())
}

async fn identify_new_face_cluster(
    cli: &Cli,
    graph: &Neo4jClient,
    labeler: &ClusterLabelProcessor,
    cluster: &VectorCluster,
) -> anyhow::Result<()> {
    if graph
        .face_cluster_has_identity_run(&cluster.cluster_id)
        .await
        .with_context(|| format!("failed checking identity for {}", cluster.cluster_id))?
    {
        debug!(cluster_id = %cluster.cluster_id, "face cluster already has identity pass");
        return Ok(());
    }

    let point_ids = cluster
        .members
        .iter()
        .map(|member| member.point_id.clone())
        .collect::<Vec<_>>();
    let items = graph
        .vector_cluster_items(&cluster.collection, &point_ids, cli.label_item_limit)
        .await
        .with_context(|| format!("failed loading source items for {}", cluster.cluster_id))?;
    if items.is_empty() {
        warn!(
            cluster_id = %cluster.cluster_id,
            "face cluster had no graph items for identity extraction"
        );
        return Ok(());
    }

    let identity = labeler
        .identify_face_cluster(cluster, &items)
        .await
        .with_context(|| format!("failed extracting identity for {}", cluster.cluster_id))?;
    graph
        .attach_face_identity(cluster, &labeler.llm_model, &items, identity.as_ref())
        .await
        .with_context(|| format!("failed attaching face identity for {}", cluster.cluster_id))?;
    info!(
        cluster_id = %cluster.cluster_id,
        source_count = items.len(),
        identity = identity.as_ref().map(|identity| identity.name.as_str()).unwrap_or(""),
        "attached face identity pass"
    );
    Ok(())
}

async fn identify_new_voice_cluster(
    cli: &Cli,
    graph: &Neo4jClient,
    labeler: &ClusterLabelProcessor,
    cluster: &VectorCluster,
) -> anyhow::Result<()> {
    if graph
        .voice_cluster_has_identity_run(&cluster.cluster_id)
        .await
        .with_context(|| format!("failed checking identity for {}", cluster.cluster_id))?
    {
        debug!(cluster_id = %cluster.cluster_id, "voice cluster already has identity pass");
        return Ok(());
    }

    let point_ids = cluster
        .members
        .iter()
        .map(|member| member.point_id.clone())
        .collect::<Vec<_>>();
    let items = graph
        .vector_cluster_items(&cluster.collection, &point_ids, cli.label_item_limit)
        .await
        .with_context(|| format!("failed loading source items for {}", cluster.cluster_id))?;
    if items.is_empty() {
        warn!(
            cluster_id = %cluster.cluster_id,
            "voice cluster had no graph items for identity extraction"
        );
        return Ok(());
    }

    let identity = labeler
        .identify_voice_cluster(cluster, &items)
        .await
        .with_context(|| format!("failed extracting identity for {}", cluster.cluster_id))?;
    graph
        .attach_voice_identity(cluster, &labeler.llm_model, &items, identity.as_ref())
        .await
        .with_context(|| format!("failed attaching voice identity for {}", cluster.cluster_id))?;
    info!(
        cluster_id = %cluster.cluster_id,
        source_count = items.len(),
        identity = identity.as_ref().map(|identity| identity.name.as_str()).unwrap_or(""),
        "attached voice identity pass"
    );
    Ok(())
}

struct ClusterLabelProcessor {
    doer: lingproc::OllamaProvider,
    llm_model: String,
}

impl ClusterLabelProcessor {
    async fn identify_face_cluster(
        &self,
        _cluster: &VectorCluster,
        items: &[GraphClusterItem],
    ) -> anyhow::Result<Option<GraphFaceIdentityLabel>> {
        let raw_text = self
            .doer
            .follow(LlmInstruction {
                command: face_identity_prompt(items),
                images: Vec::new(),
            })
            .await?
            .to_string();
        let Some(name) = normalize_face_identity(&raw_text) else {
            return Ok(None);
        };
        Ok(Some(GraphFaceIdentityLabel {
            identity_id: format!("identity:person:{}", identity_key(&name)),
            name,
        }))
    }

    async fn identify_voice_cluster(
        &self,
        _cluster: &VectorCluster,
        items: &[GraphClusterItem],
    ) -> anyhow::Result<Option<GraphVoiceIdentityLabel>> {
        let raw_text = self
            .doer
            .follow(LlmInstruction {
                command: voice_identity_prompt(items),
                images: Vec::new(),
            })
            .await?
            .to_string();
        let Some(name) = normalize_face_identity(&raw_text) else {
            return Ok(None);
        };
        Ok(Some(GraphVoiceIdentityLabel {
            identity_id: format!("identity:person:{}", identity_key(&name)),
            name,
        }))
    }
}

fn face_identity_prompt(items: &[GraphClusterItem]) -> String {
    let entries = items
        .iter()
        .map(cluster_prompt_item)
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "You identify recurring human faces from context.\n\n\
         You've seen this face several times. Here is the context of when the face was seen:\n{}\n\n\
         Who is the face these have in common? Answer with only the person's name. \
         If the context does not identify the person, answer exactly UNKNOWN. \
         Do not add commentary, uncertainty, punctuation, ids, timestamps, vectors, embeddings, clusters, or implementation details.",
        entries
    )
}

fn voice_identity_prompt(items: &[GraphClusterItem]) -> String {
    let entries = items
        .iter()
        .map(cluster_prompt_item)
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "You identify recurring human voices from context.\n\n\
         You've heard this voice several times. Here is the context of when the voice was heard:\n{}\n\n\
         Who is the voice these have in common? Answer with only the person's name. \
         If the context does not identify the person, answer exactly UNKNOWN. \
         Do not add commentary, uncertainty, punctuation, ids, timestamps, vectors, embeddings, clusters, or implementation details.",
        entries
    )
}

fn normalize_face_identity(raw_text: &str) -> Option<String> {
    let trimmed = trim_label_punctuation(raw_text.trim().trim_matches('"')).trim();
    let (without_emojis, _) = psyche::extract_emojis(trimmed);
    let name = trim_label_punctuation(without_emojis.trim()).trim();
    if name.is_empty()
        || name.eq_ignore_ascii_case("unknown")
        || name.eq_ignore_ascii_case("i don't know")
        || name.eq_ignore_ascii_case("cannot tell")
        || name.eq_ignore_ascii_case("can't tell")
    {
        None
    } else {
        Some(name.to_string())
    }
}

fn identity_key(name: &str) -> String {
    let mut key = String::new();
    let mut last_dash = false;
    for ch in name.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            key.push(ch);
            last_dash = false;
        } else if !last_dash && !key.is_empty() {
            key.push('-');
            last_dash = true;
        }
    }
    while key.ends_with('-') {
        key.pop();
    }
    if key.is_empty() {
        "unknown".into()
    } else {
        key
    }
}

fn trim_label_punctuation(text: &str) -> &str {
    text.trim()
        .trim_matches('"')
        .trim_end_matches(['.', '!', '?', ':', ';'])
        .trim()
}

fn cluster_prompt_item(item: &GraphClusterItem) -> String {
    let labels = item
        .labels
        .iter()
        .filter(|label| label.as_str() != "GraphNode")
        .cloned()
        .collect::<Vec<_>>()
        .join(",");
    let mut line = format!(
        "- {} {}",
        if labels.is_empty() { "Item" } else { &labels },
        truncate_for_prompt(&item.text, 500)
    );
    if !item.stimuli.is_empty() {
        line.push_str(" (stimuli: ");
        line.push_str(
            &item
                .stimuli
                .iter()
                .map(|stimulus| truncate_for_prompt(stimulus, 180))
                .collect::<Vec<_>>()
                .join("; "),
        );
        line.push(')');
    }
    if !item.edges.is_empty() {
        line.push_str(" (edges: ");
        line.push_str(
            &item
                .edges
                .iter()
                .map(|edge| truncate_for_prompt(edge, 180))
                .collect::<Vec<_>>()
                .join("; "),
        );
        line.push(')');
    }
    if !item.neighbors.is_empty() {
        line.push_str(" (neighbors: ");
        line.push_str(
            &item
                .neighbors
                .iter()
                .map(|neighbor| truncate_for_prompt(neighbor, 180))
                .collect::<Vec<_>>()
                .join("; "),
        );
        line.push(')');
    }
    line
}

fn truncate_for_prompt(value: &str, max_chars: usize) -> String {
    let mut output = String::new();
    for ch in value.chars().take(max_chars) {
        output.push(ch);
    }
    if value.chars().count() > max_chars {
        output.push_str("...");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cluster_prompt_item_omits_graphnode_label_and_includes_stimuli() {
        let item = GraphClusterItem {
            vector_id: "qdrant:memories:point-1".into(),
            node_id: "impression:1".into(),
            labels: vec!["GraphNode".into(), "Impression".into()],
            text: "impression: someone is talking about coffee".into(),
            stimuli: vec!["text: coffee is ready".into()],
            edges: vec!["-[:HAS_STIMULUS]-> stimulus:1".into()],
            neighbors: vec!["TextObservation text: coffee is ready".into()],
        };

        assert_eq!(
            cluster_prompt_item(&item),
            "- Impression impression: someone is talking about coffee (stimuli: text: coffee is ready) (edges: -[:HAS_STIMULUS]-> stimulus:1) (neighbors: TextObservation text: coffee is ready)"
        );
    }

    #[test]
    fn selected_collections_defaults_to_known_vector_collections() {
        assert_eq!(
            selected_collections(&[]),
            qdrant_vector_collections()
                .iter()
                .map(|collection| (*collection).to_string())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn selected_collections_accepts_csv_values_and_deduplicates() {
        assert_eq!(
            selected_collections(&[
                "memories,image_descriptions".into(),
                "memories".into(),
                " voices ".into(),
            ]),
            vec!["memories", "image_descriptions", "voices"]
        );
    }

    #[test]
    fn face_identity_prompt_requests_person_name_or_unknown() {
        let item = GraphClusterItem {
            vector_id: "qdrant:faces:point-1".into(),
            node_id: "face:1".into(),
            labels: vec!["FaceInstance".into()],
            text: "face instance detected".into(),
            stimuli: Vec::new(),
            edges: Vec::new(),
            neighbors: vec!["TextObservation text: Anna entered the room".into()],
        };

        let prompt = face_identity_prompt(&[item]);

        assert!(prompt.contains("You've seen this face several times"));
        assert!(prompt.contains("Who is the face these have in common?"));
        assert!(prompt.contains("Answer with only the person's name"));
        assert!(prompt.contains("answer exactly UNKNOWN"));
        assert!(prompt.contains("Anna entered the room"));
    }

    #[test]
    fn voice_identity_prompt_requests_person_name_or_unknown() {
        let item = GraphClusterItem {
            vector_id: "qdrant:voices:point-1".into(),
            node_id: "voice-signature:speaker:1".into(),
            labels: vec!["VoiceSignature".into()],
            text: "voice signature: f0 150 Hz, speech rate 4.5".into(),
            stimuli: vec!["audio: Anna said hello".into()],
            edges: Vec::new(),
            neighbors: vec!["AudioClip audio: Anna said hello".into()],
        };

        let prompt = voice_identity_prompt(&[item]);

        assert!(prompt.contains("You've heard this voice several times"));
        assert!(prompt.contains("Who is the voice these have in common?"));
        assert!(prompt.contains("Answer with only the person's name"));
        assert!(prompt.contains("answer exactly UNKNOWN"));
        assert!(prompt.contains("Anna said hello"));
    }

    #[test]
    fn normalize_face_identity_skips_unknown_answers() {
        assert_eq!(normalize_face_identity("UNKNOWN"), None);
        assert_eq!(normalize_face_identity("I don't know."), None);
        assert_eq!(normalize_face_identity("Anna."), Some("Anna".into()));
    }
}

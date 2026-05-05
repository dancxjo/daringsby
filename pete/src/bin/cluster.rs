use std::time::Duration;

use anyhow::Context;
use clap::Parser;
use dotenvy::dotenv;
use lingproc::{Doer, LlmInstruction};
use pete::{EventBus, init_logging, ollama_provider_from_args};
use psyche::{
    GraphClusterItem, GraphClusterTheme, Neo4jClient, QdrantClient, VectorCluster,
    find_vector_clusters, qdrant_vector_collections,
};
use tokio::time::{MissedTickBehavior, interval};
use tracing::{debug, error, info, warn};

const ALGORITHM: &str = "cosine-threshold-components/v1";

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Continuously find vector clusters, link them into Neo4j, and extract cluster themes"
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
    /// Model name to use for cluster theme extraction.
    #[arg(long, env = "WITS_MODEL", default_value = "gpt-oss")]
    wits_model: String,
    /// Maximum graph items to present to the LLM for each cluster theme.
    #[arg(long, env = "CLUSTER_THEME_ITEM_LIMIT", default_value_t = 24)]
    theme_item_limit: usize,
    /// Delay between cluster discovery passes.
    #[arg(long, env = "CLUSTER_POLL_MS", default_value_t = 5000)]
    poll_ms: u64,
    /// Run one clustering pass and exit.
    #[arg(long)]
    once: bool,
    /// Print results without writing cluster nodes to Neo4j or extracting themes.
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
    let themer = if cli.dry_run {
        None
    } else {
        Some(ClusterThemeProcessor {
            doer: ollama_provider_from_args(&cli.wits_host, &cli.wits_model)?,
            llm_model: cli.wits_model.clone(),
        })
    };

    if cli.once || cli.dry_run {
        run_cluster_pass(&cli, &qdrant, &graph, themer.as_ref()).await?;
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
        if let Err(err) = run_cluster_pass(&cli, &qdrant, &graph, themer.as_ref()).await {
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
    themer: Option<&ClusterThemeProcessor>,
) -> anyhow::Result<()> {
    let collections = selected_collections(&cli.collection);
    let skip_missing_collections = cli.collection.is_empty();

    for collection in collections {
        run_cluster_collection(
            cli,
            qdrant,
            graph,
            themer,
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
    themer: Option<&ClusterThemeProcessor>,
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

    if let Some(themer) = themer {
        theme_new_clusters(cli, graph, themer, &clusters).await?;
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

async fn theme_new_clusters(
    cli: &Cli,
    graph: &Neo4jClient,
    themer: &ClusterThemeProcessor,
    clusters: &[VectorCluster],
) -> anyhow::Result<()> {
    for cluster in clusters {
        if graph
            .vector_cluster_has_theme(&cluster.cluster_id)
            .await
            .with_context(|| format!("failed checking theme for {}", cluster.cluster_id))?
        {
            debug!(cluster_id = %cluster.cluster_id, "cluster already has a theme");
            continue;
        }

        let point_ids = cluster
            .members
            .iter()
            .map(|member| member.point_id.clone())
            .collect::<Vec<_>>();
        let items = graph
            .vector_cluster_items(&cluster.collection, &point_ids, cli.theme_item_limit)
            .await
            .with_context(|| format!("failed loading source items for {}", cluster.cluster_id))?;
        if items.is_empty() {
            warn!(
                cluster_id = %cluster.cluster_id,
                "cluster had no graph items for theme extraction"
            );
            continue;
        }

        let theme = themer
            .theme_cluster(cluster, &items)
            .await
            .with_context(|| format!("failed extracting theme for {}", cluster.cluster_id))?;
        graph
            .attach_vector_cluster_theme(cluster, &themer.llm_model, &items, &theme)
            .await
            .with_context(|| format!("failed attaching theme for {}", cluster.cluster_id))?;
        info!(
            cluster_id = %cluster.cluster_id,
            theme_id = %theme.theme_id,
            source_count = items.len(),
            theme = %theme.text,
            "attached cluster theme"
        );
    }
    Ok(())
}

struct ClusterThemeProcessor {
    doer: lingproc::OllamaProvider,
    llm_model: String,
}

impl ClusterThemeProcessor {
    async fn theme_cluster(
        &self,
        cluster: &VectorCluster,
        items: &[GraphClusterItem],
    ) -> anyhow::Result<GraphClusterTheme> {
        let raw_text = self
            .doer
            .follow(LlmInstruction {
                command: cluster_theme_prompt(cluster, items),
                images: Vec::new(),
            })
            .await?
            .to_string();
        let text = normalize_cluster_theme(&raw_text);
        anyhow::ensure!(!text.is_empty(), "cluster theme model returned empty text");
        Ok(GraphClusterTheme {
            theme_id: format!("theme:{}", cluster.cluster_id),
            text,
        })
    }
}

fn cluster_theme_prompt(cluster: &VectorCluster, items: &[GraphClusterItem]) -> String {
    let entries = items
        .iter()
        .map(cluster_prompt_item)
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "You extract terse real-world theme labels from related memories and perceptions.\n\n\
         The following entries are memories or perceptions whose embeddings are near each other. \
         Each entry may include supporting stimuli, graph edges, and nearby graph nodes for context. \
         Treat labels like Vector, Cluster, Impression, SpeechSegment, AudioClip, and ImageDescription as implementation details, not as the topic.\n\
         What is the common real-world theme among these items? Answer with only the theme itself as a concise noun phrase. \
         Do not write a complete sentence. Do not add commentary, emotion, sentence-ending punctuation, vectors, embeddings, clusters, graph ids, or implementation details.\n\n\
         Collection: {}\n\
         Cluster mean similarity: {:.3}\n\
         Entries:\n{}",
        cluster.collection, cluster.mean_similarity, entries
    )
}

fn normalize_cluster_theme(raw_text: &str) -> String {
    let trimmed = raw_text.trim().trim_matches('"').trim();
    let (without_emojis, _) = psyche::extract_emojis(trimmed);
    let without_wrappers =
        strip_cluster_theme_sentence(trim_theme_punctuation(without_emojis.trim()));
    trim_theme_punctuation(without_wrappers).to_string()
}

fn strip_cluster_theme_sentence(text: &str) -> &str {
    let mut theme = text.trim();
    for prefix in [
        "the common theme is ",
        "the shared theme is ",
        "the theme is ",
        "the consistent topic is ",
        "the topic is ",
        "common theme: ",
        "shared theme: ",
        "theme: ",
        "topic: ",
    ] {
        if let Some(rest) = strip_prefix_ignore_ascii_case(theme, prefix) {
            theme = rest.trim();
            break;
        }
    }

    for suffix in [
        " is the consistent topic of these transcriptions",
        " is the consistent topic of these entries",
        " is the consistent topic of these items",
        " is the common topic of these transcriptions",
        " is the common topic of these entries",
        " is the common topic of these items",
        " is the common theme",
        " is the shared theme",
        " is the topic",
    ] {
        if let Some(rest) = strip_suffix_ignore_ascii_case(theme, suffix) {
            theme = rest.trim();
            break;
        }
    }

    theme
}

fn strip_prefix_ignore_ascii_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let possible_prefix = text.get(..prefix.len())?;
    possible_prefix
        .eq_ignore_ascii_case(prefix)
        .then(|| &text[prefix.len()..])
}

fn strip_suffix_ignore_ascii_case<'a>(text: &'a str, suffix: &str) -> Option<&'a str> {
    let suffix_start = text.len().checked_sub(suffix.len())?;
    let possible_suffix = text.get(suffix_start..)?;
    possible_suffix
        .eq_ignore_ascii_case(suffix)
        .then(|| &text[..suffix_start])
}

fn trim_theme_punctuation(text: &str) -> &str {
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
    use psyche::VectorClusterMember;

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
    fn cluster_theme_prompt_requests_common_theme() {
        let cluster = VectorCluster {
            cluster_id: "cluster:1".into(),
            collection: "memories".into(),
            threshold: 0.86,
            centroid: vec![1.0, 0.0],
            mean_similarity: 0.91,
            members: vec![VectorClusterMember {
                point_id: "point-1".into(),
                average_similarity: 0.91,
            }],
        };
        let item = GraphClusterItem {
            vector_id: "qdrant:memories:point-1".into(),
            node_id: "impression:1".into(),
            labels: vec!["Impression".into()],
            text: "impression: coffee is brewing".into(),
            stimuli: Vec::new(),
            edges: Vec::new(),
            neighbors: Vec::new(),
        };

        let prompt = cluster_theme_prompt(&cluster, &[item]);

        assert!(prompt.contains("terse real-world theme labels"));
        assert!(prompt.contains("common real-world theme"));
        assert!(prompt.contains("Answer with only the theme itself as a concise noun phrase"));
        assert!(prompt.contains("Do not write a complete sentence"));
        assert!(!prompt.contains("You intersperse emojis"));
        assert!(!prompt.contains("emoji"));
        assert!(!prompt.contains("one short sentence"));
        assert!(prompt.contains("vectors, embeddings, clusters"));
        assert!(prompt.contains("coffee is brewing"));
    }

    #[test]
    fn normalize_cluster_theme_removes_sentence_wrapper_and_emoji() {
        assert_eq!(
            normalize_cluster_theme(
                "A virus outbreak on a boat is the consistent topic of these transcriptions. 😟"
            ),
            "A virus outbreak on a boat"
        );
    }

    #[test]
    fn normalize_cluster_theme_removes_theme_prefix_and_punctuation() {
        assert_eq!(
            normalize_cluster_theme("\"The common theme is coffee brewing.\""),
            "coffee brewing"
        );
    }
}

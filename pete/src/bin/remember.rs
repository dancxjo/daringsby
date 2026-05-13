use std::collections::HashMap;
use std::time::Duration;

use anyhow::Context;
use clap::Parser;
use dotenvy::dotenv;
use lingproc::{Doer, LlmInstruction, Vectorizer};
use pete::{EventBus, init_logging, ollama_provider_from_args};
use psyche::{
    GraphClusterItem, GraphSensationTimelineItem, GraphSnapshot, Neo4jClient, QdrantClient,
    with_default_system_prompt,
};
use tokio::time::{MissedTickBehavior, interval};
use tracing::{debug, error, info, trace, warn};

const MEMORY_COLLECTION: &str = "memories";

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Recall related memories from recent sensations and record them as new sensations"
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
    /// URL of the Qdrant vector store.
    #[arg(long, env = "QDRANT_URL", default_value = "http://localhost:6333")]
    qdrant_url: String,
    /// URL of the embeddings Ollama server.
    #[arg(
        long,
        env = "EMBEDDINGS_HOST",
        default_value = "http://localhost:11434"
    )]
    embeddings_host: String,
    /// Embedding model name used for memory vectors.
    #[arg(long, env = "EMBEDDINGS_MODEL", default_value = "embeddinggemma")]
    embeddings_model: String,
    /// URL of the remembering Ollama server.
    #[arg(
        long = "remember-host",
        alias = "wits-host",
        env = "REMEMBER_HOST",
        default_value = "http://localhost:11434"
    )]
    remember_host: String,
    /// Model name to use for remembering.
    #[arg(
        long = "remember-model",
        alias = "wits-model",
        env = "REMEMBER_MODEL",
        default_value = "gpt-oss"
    )]
    remember_model: String,
    /// Maximum newly formed sensations to combine into one remembering query.
    #[arg(long, env = "REMEMBER_RECENT_LIMIT", default_value_t = 8)]
    recent_limit: usize,
    /// Maximum nearest memory vectors to retrieve.
    #[arg(long, env = "REMEMBER_MEMORY_LIMIT", default_value_t = 6)]
    memory_limit: usize,
    /// Minimum vector score for related memories; set to 0 to disable the threshold.
    #[arg(long, env = "REMEMBER_SCORE_THRESHOLD", default_value_t = 0.0)]
    score_threshold: f32,
    /// Graph hops to include around each retrieved memory neighbor.
    #[arg(long, env = "REMEMBER_GRAPH_HOPS", default_value_t = 2)]
    graph_hops: usize,
    /// Maximum graph nodes to include per retrieved neighbor context.
    #[arg(long, env = "REMEMBER_GRAPH_CONTEXT_LIMIT", default_value_t = 24)]
    graph_context_limit: usize,
    /// Delay between graph polling attempts.
    #[arg(long, env = "REMEMBER_POLL_MS", default_value_t = 5000)]
    poll_ms: u64,
    /// Process at most one batch and exit.
    #[arg(long)]
    once: bool,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let (bus, _user_rx) = EventBus::new();
    init_logging(bus.log_sender());
    dotenv().ok();

    let cli = Cli::parse();
    let graph = Neo4jClient::new(
        cli.neo4j_uri.clone(),
        cli.neo4j_user.clone(),
        cli.neo4j_pass.clone(),
    );
    let qdrant = QdrantClient::new(cli.qdrant_url.clone());
    let doer = ollama_provider_from_args(&cli.remember_host, &cli.remember_model)?;
    let vectorizer = ollama_provider_from_args(&cli.embeddings_host, &cli.embeddings_model)?;
    let processor = RememberProcessor {
        doer,
        vectorizer,
        llm_model: cli.remember_model.clone(),
    };

    if cli.once {
        process_next_batch(&graph, &qdrant, &processor, &cli).await?;
        return Ok(());
    }

    let mut ticker = interval(Duration::from_millis(cli.poll_ms.max(500)));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    info!(
        recent_limit = cli.recent_limit,
        memory_limit = cli.memory_limit,
        graph_hops = cli.graph_hops.clamp(1, 2),
        "remembering loop started"
    );
    loop {
        ticker.tick().await;
        if let Err(err) = process_next_batch(&graph, &qdrant, &processor, &cli).await {
            error!(error = %err, "remembering loop iteration failed");
        }
    }
}

async fn process_next_batch(
    graph: &Neo4jClient,
    qdrant: &QdrantClient,
    processor: &RememberProcessor,
    cli: &Cli,
) -> anyhow::Result<()> {
    let latest_remembrance_at = graph
        .latest_remembrance_sensation_at()
        .await
        .context("failed to load latest remembrance sensation timestamp")?;
    let sources = graph
        .latest_sensations_for_remembering(latest_remembrance_at.as_deref(), cli.recent_limit)
        .await
        .context("failed to load recent sensations for remembering")?;
    if sources.is_empty() {
        trace!("no new sensations found for remembering");
        return Ok(());
    }

    info!(
        source_count = sources.len(),
        "looking for memories related to latest sensations"
    );
    let result = processor
        .remember(graph, qdrant, &sources, cli)
        .await
        .context("failed to produce remembrance")?;

    graph
        .attach_remembrance(
            &sources,
            &result.related_memories,
            &processor.llm_model,
            &result.how,
        )
        .await
        .context("failed to store remembrance sensation")?;
    info!(
        source_count = sources.len(),
        related_count = result.related_memories.len(),
        how = %result.how,
        "stored remembrance sensation"
    );
    Ok(())
}

struct RememberProcessor {
    doer: lingproc::OllamaProvider,
    vectorizer: lingproc::OllamaProvider,
    llm_model: String,
}

struct RememberResult {
    how: String,
    related_memories: Vec<GraphClusterItem>,
}

impl RememberProcessor {
    async fn remember(
        &self,
        graph: &Neo4jClient,
        qdrant: &QdrantClient,
        sources: &[GraphSensationTimelineItem],
        cli: &Cli,
    ) -> anyhow::Result<RememberResult> {
        let query = sources
            .iter()
            .map(|item| item.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let vector = self
            .vectorizer
            .vectorize(&query)
            .await
            .context("failed to embed latest sensation text")?;
        anyhow::ensure!(
            !vector.is_empty(),
            "embedding model returned no query vector"
        );

        let threshold = (cli.score_threshold > 0.0).then_some(cli.score_threshold);
        let neighbors = qdrant
            .search_vectors(MEMORY_COLLECTION, &vector, cli.memory_limit, threshold)
            .await
            .context("failed to search memory vectors")?;
        anyhow::ensure!(
            !neighbors.is_empty(),
            "no related memory vectors matched latest sensations"
        );

        let point_ids = neighbors
            .iter()
            .map(|neighbor| neighbor.point_id.clone())
            .collect::<Vec<_>>();
        let score_by_vector = neighbors
            .iter()
            .map(|neighbor| {
                (
                    format!("qdrant:{MEMORY_COLLECTION}:{}", neighbor.point_id),
                    neighbor.score,
                )
            })
            .collect::<HashMap<_, _>>();
        let mut related_memories = graph
            .vector_cluster_items(MEMORY_COLLECTION, &point_ids, cli.memory_limit)
            .await
            .context("failed to load related memory graph items")?;
        related_memories.sort_by(|left, right| {
            score_by_vector
                .get(&right.vector_id)
                .partial_cmp(&score_by_vector.get(&left.vector_id))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.node_id.cmp(&right.node_id))
        });
        related_memories.dedup_by(|left, right| {
            left.node_id == right.node_id && left.vector_id == right.vector_id
        });
        anyhow::ensure!(
            !related_memories.is_empty(),
            "memory vector matches had no graph context"
        );

        let graph_contexts = related_graph_contexts(
            graph,
            &related_memories,
            cli.graph_hops,
            cli.graph_context_limit,
        )
        .await;
        let prompt = remembrance_prompt(sources, &related_memories, &graph_contexts);
        let raw_text = self
            .doer
            .follow(LlmInstruction {
                command: prompt,
                images: Vec::new(),
            })
            .await
            .context("remembering model failed")?;
        let how = remember_how(&raw_text)?;
        Ok(RememberResult {
            how,
            related_memories,
        })
    }
}

async fn related_graph_contexts(
    graph: &Neo4jClient,
    related_memories: &[GraphClusterItem],
    graph_hops: usize,
    graph_context_limit: usize,
) -> Vec<(String, GraphSnapshot)> {
    let mut contexts = Vec::new();
    for item in related_memories {
        match graph
            .graph_neighbors(&item.node_id, graph_hops.clamp(1, 2), graph_context_limit)
            .await
        {
            Ok(snapshot) => contexts.push((item.node_id.clone(), snapshot)),
            Err(err) => warn!(
                node_id = %item.node_id,
                error = %err,
                "failed to load related memory graph neighborhood"
            ),
        }
    }
    contexts
}

fn remembrance_prompt(
    sources: &[GraphSensationTimelineItem],
    related_memories: &[GraphClusterItem],
    graph_contexts: &[(String, GraphSnapshot)],
) -> String {
    with_default_system_prompt(format!(
        "Internal remembering task: the current sensations below are retrieval cues, and the memory fragments are accumulated remembered material. Compose the text for a new sensation's how field.\n\
         The result must stand alone after the prefix \"I remember:\". It should sound like a compact remembered impression from Pete, not like a report about data processing.\n\
         Use concrete remembered people, places, conversations, actions, or situations from the fragments. If the fragments are mixed, synthesize the most coherent remembered gist. If the memory is uncertain, say what is uncertain in first person.\n\
         Do not mention logs, snippets, graph context, clusters, nodes, vectors, embeddings, retrieval rankings, databases, prompts, labels, ids, timestamps, or the fact that this is an internal task. Do not answer yes/no. Do not produce markdown, headings, bullets, commands, tool output, or emoji.\n\
         Return exactly one or two concise first-person sentences, without the \"I remember:\" prefix.\n\n\
         Current retrieval cues:\n{}\n\n\
         Core memory fragments:\n{}\n\n\
         Nearby remembered details from two graph hops:\n{}",
        format_sources(sources),
        format_related_memories(related_memories),
        format_graph_contexts(graph_contexts)
    ))
}

fn format_sources(sources: &[GraphSensationTimelineItem]) -> String {
    sources
        .iter()
        .map(|item| format!("- [{}] {}", item.kind, truncate_for_prompt(&item.text, 500)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_related_memories(related_memories: &[GraphClusterItem]) -> String {
    related_memories
        .iter()
        .flat_map(|item| {
            std::iter::once(clean_memory_text(&item.text))
                .chain(item.stimuli.iter().map(|value| clean_memory_text(value)))
        })
        .filter(|text| usable_memory_text(text))
        .take(16)
        .map(|text| format!("- {}", truncate_for_prompt(&text, 420)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_graph_contexts(graph_contexts: &[(String, GraphSnapshot)]) -> String {
    if graph_contexts.is_empty() {
        return "(no graph neighborhoods loaded)".into();
    }
    let mut details = Vec::new();
    for (_, snapshot) in graph_contexts {
        for node in &snapshot.nodes {
            let text = clean_memory_text(&node_text(&node.properties));
            if usable_memory_text(&text) && !details.contains(&text) {
                details.push(text);
            }
            if details.len() >= 24 {
                break;
            }
        }
        if details.len() >= 24 {
            break;
        }
    }
    if details.is_empty() {
        return "(no usable nearby remembered details)".into();
    }
    details
        .iter()
        .map(|text| format!("- {}", truncate_for_prompt(text, 360)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn node_text(properties: &serde_json::Value) -> String {
    for key in [
        "how",
        "summary",
        "text",
        "transcript",
        "object_label",
        "name",
    ] {
        if let Some(value) = properties.get(key).and_then(serde_json::Value::as_str) {
            if !value.trim().is_empty() {
                return truncate_for_prompt(value, 300);
            }
        }
    }
    "(no compact text)".into()
}

fn clean_memory_text(value: &str) -> String {
    let mut text = value.trim();
    for prefix in [
        "awareness:",
        "impression:",
        "speech:",
        "transcription:",
        "vision:",
        "audio:",
        "text:",
        "object:",
        "voice signature:",
        "voice sample:",
    ] {
        if let Some(rest) = text.strip_prefix(prefix) {
            text = rest.trim();
            break;
        }
    }
    text.lines().next().unwrap_or_default().trim().to_string()
}

fn usable_memory_text(value: &str) -> bool {
    let text = value.trim();
    if text.is_empty() || text == "(no compact text)" {
        return false;
    }
    let lower = text.to_lowercase();
    ![
        "qdrant:",
        "sha256:",
        "graphnode",
        "provided log",
        "provided context",
        "structured breakdown",
        "cluster membership",
        "vector",
        "embedding",
        "database",
        "prompt",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn remember_how(raw_text: &str) -> anyhow::Result<String> {
    let trimmed = raw_text
        .trim()
        .trim_matches(|ch| ch == '"' || ch == '\'' || ch == '`')
        .trim();
    let content = trimmed
        .strip_prefix("I remember:")
        .or_else(|| trimmed.strip_prefix("I remember"))
        .unwrap_or(trimmed)
        .trim()
        .trim_start_matches(':')
        .trim();
    let content = first_two_sentences(content);
    let content =
        common::non_empty_model_text(&content).context("remembering model returned empty text")?;
    anyhow::ensure!(
        usable_memory_text(content) && !content.to_lowercase().starts_with("yes,"),
        "remembering model returned implementation text instead of a remembered impression: {content}"
    );
    debug!(raw = %raw_text.trim(), content = %content, "cleaned remembrance text");
    Ok(format!("I remember: {content}"))
}

fn first_two_sentences(text: &str) -> String {
    let mut sentence_count = 0;
    let mut end_byte = text.len();
    for (index, ch) in text.char_indices() {
        if matches!(ch, '.' | '!' | '?') {
            sentence_count += 1;
            if sentence_count == 2 {
                end_byte = index + ch.len_utf8();
                break;
            }
        }
    }
    text[..end_byte].trim().to_string()
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
    fn remember_how_adds_prefix_and_limits_to_two_sentences() {
        let how = remember_how("I remember: first. second. third.").unwrap();
        assert_eq!(how, "I remember: first. second.");
    }

    #[test]
    fn remember_how_rejects_implementation_text() {
        let err = remember_how("Yes, the cluster membership for the user has been updated.")
            .expect_err("implementation text should be rejected");
        assert!(
            err.to_string()
                .contains("instead of a remembered impression")
        );
    }

    #[test]
    fn related_memory_prompt_hides_graph_mechanics() {
        let item = GraphClusterItem {
            vector_id: "qdrant:memories:point-1".into(),
            node_id: "awareness:sha256:abc".into(),
            labels: vec!["GraphNode".into(), "Awareness".into()],
            text: "awareness: I hear Travis talking about memory storage.".into(),
            stimuli: vec!["text: Travis says he is my creator.".into()],
            edges: Vec::new(),
            neighbors: Vec::new(),
        };
        let prompt = remembrance_prompt(&[], &[item], &[]);
        assert!(prompt.contains("I hear Travis talking about memory storage."));
        assert!(prompt.contains("Travis says he is my creator."));
        assert!(!prompt.contains("qdrant:"));
        assert!(!prompt.contains("sha256:"));
        assert!(!prompt.contains("score"));
        assert!(!prompt.contains("GraphNode"));
    }
}

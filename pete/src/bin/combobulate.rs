use std::time::Duration;

use anyhow::Context;
use clap::Parser;
use dotenvy::dotenv;
use lingproc::{Doer, LlmInstruction, Vectorizer};
use pete::{EventBus, init_logging, ollama_provider_from_args};
use psyche::{
    GraphAwareness, GraphTimelineItem, GraphTimelineWindow, Neo4jClient, QdrantClient,
    with_default_system_prompt,
};
use tokio::time::{MissedTickBehavior, interval};
use tracing::{debug, error, info};

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Summarize recent graph timelines with an LLM and embed the awareness text"
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
    /// URL of the wits Ollama server.
    #[arg(long, env = "WITS_HOST", default_value = "http://localhost:11434")]
    wits_host: String,
    /// Model name to use for combobulation.
    #[arg(long, env = "WITS_MODEL", default_value = "gemma3")]
    wits_model: String,
    /// URL of the embeddings Ollama server.
    #[arg(
        long,
        env = "EMBEDDINGS_HOST",
        default_value = "http://localhost:11434"
    )]
    embeddings_host: String,
    /// Model name to use for awareness text embeddings.
    #[arg(long, env = "EMBEDDINGS_MODEL", default_value = "embeddinggemma")]
    embeddings_model: String,
    /// Number of seconds of graph history to show the LLM.
    #[arg(long, env = "COMBOBULATION_WINDOW_SECONDS", default_value_t = 30)]
    window_seconds: u64,
    /// Maximum timeline items to include in one LLM prompt.
    #[arg(long, env = "COMBOBULATION_WINDOW_LIMIT", default_value_t = 80)]
    window_limit: usize,
    /// Delay between graph polling attempts.
    #[arg(long, env = "COMBOBULATION_POLL_MS", default_value_t = 1000)]
    poll_ms: u64,
    /// Process at most one window and exit.
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
    let doer = ollama_provider_from_args(&cli.wits_host, &cli.wits_model)?;
    let vectorizer = ollama_provider_from_args(&cli.embeddings_host, &cli.embeddings_model)?;
    let processor = CombobulationProcessor {
        doer,
        vectorizer,
        llm_model: cli.wits_model,
        embedding_model: cli.embeddings_model,
    };

    if cli.once {
        process_next_window(
            &graph,
            &qdrant,
            &processor,
            cli.window_seconds,
            cli.window_limit,
        )
        .await?;
        return Ok(());
    }

    let mut ticker = interval(Duration::from_millis(cli.poll_ms.max(100)));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    info!(
        window_seconds = cli.window_seconds,
        window_limit = cli.window_limit,
        "combobulation loop started"
    );
    loop {
        ticker.tick().await;
        if let Err(err) = process_next_window(
            &graph,
            &qdrant,
            &processor,
            cli.window_seconds,
            cli.window_limit,
        )
        .await
        {
            error!(error = %err, "combobulation loop iteration failed");
        }
    }
}

async fn process_next_window(
    graph: &Neo4jClient,
    qdrant: &QdrantClient,
    processor: &CombobulationProcessor,
    window_seconds: u64,
    window_limit: usize,
) -> anyhow::Result<()> {
    let Some(window) = graph
        .latest_timeline_window_for_combobulation(window_seconds, window_limit)
        .await
        .context("failed to load latest timeline window")?
    else {
        debug!("no timeline windows found for combobulation");
        return Ok(());
    };

    if window.items.is_empty() {
        debug!(
            anchor_id = %window.anchor_id,
            "timeline window had no source events"
        );
        return Ok(());
    }

    info!(
        anchor_id = %window.anchor_id,
        source_count = window.items.len(),
        "combobulating timeline window"
    );
    let awareness = processor
        .combobulate(&window, qdrant, window_seconds)
        .await
        .with_context(|| format!("failed to combobulate timeline {}", window.anchor_id))?;
    graph
        .attach_combobulation(
            &window,
            &processor.llm_model,
            &processor.embedding_model,
            &awareness,
        )
        .await
        .with_context(|| {
            format!(
                "failed to attach combobulation for timeline {}",
                window.anchor_id
            )
        })?;
    info!(
        anchor_id = %window.anchor_id,
        awareness_id = %awareness.awareness_id,
        vector_id = %awareness.vector_id,
        embedding_len = awareness.embedding_len,
        "attached combobulation"
    );
    Ok(())
}

struct CombobulationProcessor {
    doer: lingproc::OllamaProvider,
    vectorizer: lingproc::OllamaProvider,
    llm_model: String,
    embedding_model: String,
}

impl CombobulationProcessor {
    async fn combobulate(
        &self,
        window: &GraphTimelineWindow,
        qdrant: &QdrantClient,
        window_seconds: u64,
    ) -> anyhow::Result<GraphAwareness> {
        let prompt = combobulation_prompt(window, window_seconds);
        let text = self
            .doer
            .follow(LlmInstruction {
                command: prompt,
                images: Vec::new(),
            })
            .await?
            .trim()
            .to_string();
        anyhow::ensure!(!text.is_empty(), "combobulation model returned empty text");

        let embedding = self
            .vectorizer
            .vectorize(&text)
            .await
            .context("failed to embed awareness text")?;
        anyhow::ensure!(
            !embedding.is_empty(),
            "embedding model returned no vector for timeline {}",
            window.anchor_id
        );

        let awareness_id = awareness_id(window);
        let vector_id = qdrant
            .store_vector_for_node(&text, Some(&awareness_id), &embedding)
            .await
            .context("failed to store awareness vector")?
            .to_string();

        Ok(GraphAwareness {
            awareness_id,
            text,
            vector_id,
            embedding_len: embedding.len(),
        })
    }
}

fn combobulation_prompt(window: &GraphTimelineWindow, window_seconds: u64) -> String {
    let timeline = window
        .items
        .iter()
        .map(timeline_prompt_item)
        .collect::<Vec<_>>()
        .join("\n");
    with_default_system_prompt(format!(
        "The following entries are a chronological timeline of your internal representations of real-world events happening around or to you during the last {window_seconds} seconds.\n\
         Treat labels like SpeechSegment, AudioClip, Impression, memory, and perception as evidence about the actual situation, not as the topic to describe.\n\
         What is going on right now? Summarize your current awareness in one or two grounded first-person sentences. Do not say that you are observing a timeline, recordings, entries, or a shift in conversation. Do not mention graph ids unless they are directly relevant.\n\n\
         Timeline:\n{timeline}"
    ))
}

fn timeline_prompt_item(item: &GraphTimelineItem) -> String {
    let labels = item
        .labels
        .iter()
        .filter(|label| label.as_str() != "GraphNode")
        .cloned()
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "- [{}] {} {}",
        item.occurred_at,
        if labels.is_empty() { "Event" } else { &labels },
        truncate_for_prompt(&item.text, 500)
    )
}

fn awareness_id(window: &GraphTimelineWindow) -> String {
    format!(
        "awareness:{}:{}:{}",
        window.anchor_id,
        window.anchor_at,
        window.items.len()
    )
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
    fn timeline_prompt_item_omits_graphnode_label() {
        let item = GraphTimelineItem {
            id: "speech:1".into(),
            labels: vec!["GraphNode".into(), "SpeechSegment".into()],
            text: "speech: hello".into(),
            occurred_at: "2026-05-05T12:34:56Z".into(),
        };

        assert_eq!(
            timeline_prompt_item(&item),
            "- [2026-05-05T12:34:56Z] SpeechSegment speech: hello"
        );
    }

    #[test]
    fn combobulation_prompt_includes_default_system_prompt_and_timeline() {
        let window = GraphTimelineWindow {
            anchor_id: "speech:1".into(),
            anchor_at: "2026-05-05T12:34:56Z".into(),
            items: vec![GraphTimelineItem {
                id: "speech:1".into(),
                labels: vec!["SpeechSegment".into()],
                text: "speech: hello".into(),
                occurred_at: "2026-05-05T12:34:56Z".into(),
            }],
        };

        let prompt = combobulation_prompt(&window, 30);

        assert!(prompt.contains("You are PETE"));
        assert!(prompt.contains("last 30 seconds"));
        assert!(prompt.contains("internal representations of real-world events"));
        assert!(prompt.contains("not as the topic to describe"));
        assert!(prompt.contains("Do not say that you are observing a timeline"));
        assert!(prompt.contains("SpeechSegment speech: hello"));
    }
}

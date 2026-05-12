use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use chrono::Utc;
use clap::Parser;
use dotenvy::dotenv;
use lingproc::{Doer, LlmInstruction, Vectorizer};
use pete::{EventBus, init_logging, ollama_provider_from_args};
use psyche::{
    ConversationEntry, GraphAwareness, GraphSensationTimelineItem, GraphTimelineWindow,
    Neo4jClient, QdrantClient, SENSOR_GROUNDING_RULES, Sensation, SensationGraphObserver,
    SensationObserver, WillContext, WitReport, with_default_system_prompt,
};
use tokio::time::{MissedTickBehavior, interval};
use tracing::{debug, error, info, trace};

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
    #[arg(long, env = "WITS_MODEL", default_value = "gpt-oss")]
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
    /// Number of seconds of graph history to include in one FIFO combobulation chunk.
    #[arg(long, env = "COMBOBULATION_WINDOW_SECONDS", default_value_t = 600)]
    window_seconds: u64,
    /// Maximum timeline items to include in one LLM prompt; 0 includes all sensations in the window.
    #[arg(long, env = "COMBOBULATION_WINDOW_LIMIT", default_value_t = 0)]
    window_limit: usize,
    /// Delay between graph polling attempts.
    #[arg(long, env = "COMBOBULATION_POLL_MS", default_value_t = 100)]
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
    let graph = Arc::new(Neo4jClient::new(
        cli.neo4j_uri,
        cli.neo4j_user,
        cli.neo4j_pass,
    ));
    let observer = SensationGraphObserver::new(graph.clone());
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
            &observer,
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
            &observer,
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
    observer: &SensationGraphObserver,
    processor: &CombobulationProcessor,
    window_seconds: u64,
    window_limit: usize,
) -> anyhow::Result<()> {
    let latest_combobulation_sensation_at = graph
        .latest_combobulation_sensation_at()
        .await
        .context("failed to load latest combobulation sensation timestamp")?;
    if let Some(window) = graph
        .latest_timeline_window_for_combobulation(window_seconds, window_limit)
        .await
        .context("failed to load next timeline window")?
    {
        process_window(
            graph,
            qdrant,
            observer,
            processor,
            window_seconds,
            latest_combobulation_sensation_at,
            window,
        )
        .await?;
    } else {
        trace!("no timeline windows found for combobulation");
    }
    Ok(())
}

async fn process_window(
    graph: &Neo4jClient,
    qdrant: &QdrantClient,
    observer: &SensationGraphObserver,
    processor: &CombobulationProcessor,
    window_seconds: u64,
    latest_combobulation_sensation_at: Option<String>,
    window: GraphTimelineWindow,
) -> anyhow::Result<()> {
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
    let conversation = graph
        .conversation_timeline(None, Utc::now(), 12)
        .await
        .unwrap_or_default();
    let result = processor
        .combobulate(
            &window,
            qdrant,
            window_seconds,
            latest_combobulation_sensation_at.as_deref(),
            &conversation,
        )
        .await
        .with_context(|| format!("failed to combobulate timeline {}", window.anchor_id))?;
    let awareness = result.awareness;
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
    store_combobulator_context_sensation(observer, result.report).await;
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

struct CombobulationResult {
    awareness: GraphAwareness,
    report: WitReport,
}

impl CombobulationProcessor {
    async fn combobulate(
        &self,
        window: &GraphTimelineWindow,
        qdrant: &QdrantClient,
        window_seconds: u64,
        latest_combobulation_sensation_at: Option<&str>,
        conversation: &[GraphSensationTimelineItem],
    ) -> anyhow::Result<CombobulationResult> {
        let prompt = combobulation_prompt(
            window,
            window_seconds,
            latest_combobulation_sensation_at,
            conversation,
        );
        let raw_text = self
            .doer
            .follow(LlmInstruction {
                command: prompt.clone(),
                images: Vec::new(),
            })
            .await?
            .trim()
            .to_string();
        let (text_without_emoji, emojis) = psyche::extract_emojis(&raw_text);
        let emoji = emojis.last().cloned();
        let text = if text_without_emoji.is_empty() {
            raw_text.clone()
        } else {
            text_without_emoji
        };
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

        Ok(CombobulationResult {
            awareness: GraphAwareness {
                awareness_id,
                text,
                emoji,
                vector_id,
                embedding_len: embedding.len(),
            },
            report: WitReport {
                name: "Combobulator".into(),
                prompt,
                output: raw_text,
            },
        })
    }
}

async fn store_combobulator_context_sensation(
    observer: &SensationGraphObserver,
    report: WitReport,
) {
    let context = WillContext {
        system_prompt: report.prompt.clone(),
        history: Vec::<ConversationEntry>::new(),
        report: Some(report),
    };
    let sensation = Sensation::of_at(context, Utc::now());
    observer.observe_sensation(&sensation).await;
}

fn combobulation_prompt(
    window: &GraphTimelineWindow,
    window_seconds: u64,
    latest_combobulation_sensation_at: Option<&str>,
    conversation: &[GraphSensationTimelineItem],
) -> String {
    let timeline = timeline_prompt(window);
    let conversation_context = current_conversation_prompt(conversation);
    let latest_combobulation_note = match latest_combobulation_sensation_at {
        Some(occurred_at) => {
            format!("The last recorded combobulation sensation occurred at {occurred_at}.")
        }
        None => "There is no recorded prior combobulation sensation.".to_string(),
    };
    with_default_system_prompt(format!(
        "The following entries are a chronological timeline of your next uncombobulated sensations, selected FIFO from the oldest pending sensation and bounded to {window_seconds} seconds. Each entry is already a compact summary of one source sensation, such as hearing, seeing, feeling, locating, or thinking a combobulation thought.\n\
         If there are no sensations in the timeline, infer that you must be asleep.\n\
         {latest_combobulation_note}\n\
         Treat these sensations as fragmentary, possibly contradictory, fleeting evidence about the actual situation, not as the topic to describe. Try to infer what is going on in the real world from those fragments. Some entries may be your own prior combobulation summaries looping back in as sensations; treat those as provisional, possibly stale self-context, not as fresh external evidence.\n\
         {SENSOR_GROUNDING_RULES} What is going on right now? Summarize your current awareness in one or two grounded first-person sentences, then end with exactly one emoji that reflects the tone of the moment. Keep it compact: compress repeated low-level records into the real-world gist. Do not say that you are observing a timeline, sensations, recordings, entries, a previous summary, or a shift in conversation. Do not mention graph ids, hashes, timestamps, edges, or per-detection details unless they are directly relevant.\n\n\
         Current conversation:\n{conversation_context}\n\n\
         Timeline:\n{timeline}"
    ))
}

fn current_conversation_prompt(items: &[GraphSensationTimelineItem]) -> String {
    if items.is_empty() {
        return "(no current conversation)".into();
    }
    items
        .iter()
        .map(|item| format!("- {} [{}]: {}", item.occurred_at, item.kind, item.text))
        .collect::<Vec<_>>()
        .join("\n")
}

fn timeline_timestamp(value: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|timestamp| psyche::model::localized_timestamp(timestamp.with_timezone(&chrono::Utc)))
        .unwrap_or_else(|_| value.to_string())
}

fn timeline_prompt(window: &GraphTimelineWindow) -> String {
    let from = window
        .items
        .first()
        .map(|item| item.occurred_at.as_str())
        .unwrap_or(window.anchor_at.as_str());
    let to = window
        .items
        .last()
        .map(|item| item.occurred_at.as_str())
        .unwrap_or(window.anchor_at.as_str());

    let mut current_time = String::new();
    let mut current_texts = Vec::new();
    let mut entries = Vec::new();

    for item in &window.items {
        let ts = timeline_timestamp(&item.occurred_at);
        if ts != current_time {
            if !current_texts.is_empty() {
                entries.push(format!("[{}] {}", current_time, current_texts.join(" ")));
            }
            current_time = ts;
            current_texts.clear();
        }
        current_texts.push(truncate_for_prompt(&item.text, 500));
    }
    if !current_texts.is_empty() {
        entries.push(format!("[{}] {}", current_time, current_texts.join(" ")));
    }

    let entries = entries.join("\n");
    format!(
        "Sensation timeline {} to {}\n{}",
        timeline_timestamp(from),
        timeline_timestamp(to),
        entries
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
    use psyche::GraphTimelineItem;

    fn current_conversation() -> Vec<GraphSensationTimelineItem> {
        vec![GraphSensationTimelineItem {
            id: "sensation:heard:1".into(),
            labels: vec!["GraphNode".into(), "Sensation".into()],
            kind: "text".into(),
            text: "I heard: are you awake?".into(),
            occurred_at: "2026-05-05T12:35:00Z".into(),
            formed_at: Some("2026-05-05T12:35:01Z".into()),
        }]
    }

    #[test]
    fn timeline_prompt_groups_entries_by_timestamp() {
        let window = GraphTimelineWindow {
            anchor_id: "sensation:audio:2".into(),
            anchor_at: "2026-05-05T12:34:56Z".into(),
            items: vec![
                GraphTimelineItem {
                    id: "sensation:audio:1".into(),
                    event_id: "audio:1".into(),
                    labels: vec!["Sensation".into()],
                    text: "audio sensation; transcript: hello".into(),
                    occurred_at: "2026-05-05T12:34:56.123Z".into(),
                },
                GraphTimelineItem {
                    id: "sensation:thought:1".into(),
                    event_id: "combobulation-summary:1".into(),
                    labels: vec!["Sensation".into()],
                    text: "combobulation sensation; greeting.".into(),
                    occurred_at: "2026-05-05T12:34:56.456Z".into(),
                },
            ],
        };

        let prompt = timeline_prompt(&window);
        let ts = timeline_timestamp("2026-05-05T12:34:56Z");
        assert_eq!(
            prompt,
            format!(
                "Sensation timeline {ts} to {ts}\n[{ts}] audio sensation; transcript: hello combobulation sensation; greeting."
            )
        );
    }

    #[test]
    fn combobulation_prompt_includes_default_system_prompt_and_timeline() {
        let window = GraphTimelineWindow {
            anchor_id: "speech:1".into(),
            anchor_at: "2026-05-05T12:34:56Z".into(),
            items: vec![GraphTimelineItem {
                id: "sensation:audio:1".into(),
                event_id: "audio:1".into(),
                labels: vec!["Sensation".into()],
                text: "audio sensation; transcript: hello".into(),
                occurred_at: "2026-05-05T12:34:56Z".into(),
            }],
        };

        let prompt = combobulation_prompt(
            &window,
            600,
            Some("2026-05-05T12:30:00Z"),
            &current_conversation(),
        );

        assert!(prompt.contains("You are PETE"));
        assert!(prompt.contains("next uncombobulated sensations"));
        assert!(prompt.contains("selected FIFO from the oldest pending sensation"));
        assert!(prompt.contains("bounded to 600 seconds"));
        assert!(prompt.contains("If there are no sensations in the timeline"));
        assert!(prompt.contains(
            "The last recorded combobulation sensation occurred at 2026-05-05T12:30:00Z."
        ));
        assert!(prompt.contains("chronological timeline of your next uncombobulated sensations"));
        assert!(prompt.contains("fragmentary, possibly contradictory, fleeting evidence"));
        assert!(prompt.contains("prior combobulation summaries looping back in as sensations"));
        assert!(prompt.contains("not as the topic to describe"));
        assert!(prompt.contains("not the sensor stream"));
        assert!(prompt.contains("amount, density, cadence, or mix of input modalities"));
        assert!(prompt.contains("I cannot tell what is happening yet"));
        assert!(prompt.contains("Do not say that you are observing a timeline"));
        assert!(prompt.contains("end with exactly one emoji"));
        assert!(prompt.contains("Keep it compact"));
        assert!(prompt.contains("per-detection details"));
        assert!(prompt.contains("Current conversation:"));
        assert!(prompt.contains("I heard: are you awake?"));
        assert!(prompt.contains("Timeline:"));
        let ts = timeline_timestamp("2026-05-05T12:34:56Z");
        assert!(prompt.contains(&format!(
            "Sensation timeline {ts} to {ts}\n[{ts}] audio sensation; transcript: hello"
        )));
    }

    #[test]
    fn combobulation_prompt_includes_empty_current_conversation_section() {
        let window = GraphTimelineWindow {
            anchor_id: "speech:1".into(),
            anchor_at: "2026-05-05T12:34:56Z".into(),
            items: vec![GraphTimelineItem {
                id: "sensation:audio:1".into(),
                event_id: "audio:1".into(),
                labels: vec!["Sensation".into()],
                text: "audio sensation; transcript: hello".into(),
                occurred_at: "2026-05-05T12:34:56Z".into(),
            }],
        };

        let prompt = combobulation_prompt(&window, 600, None, &[]);

        assert!(prompt.contains("Current conversation:"));
        assert!(prompt.contains("(no current conversation)"));
    }

    #[test]
    fn timeline_prompt_matches_timeline_binary_header_and_entries() {
        let window = GraphTimelineWindow {
            anchor_id: "sensation:audio:2".into(),
            anchor_at: "2026-05-05T12:34:57Z".into(),
            items: vec![
                GraphTimelineItem {
                    id: "sensation:audio:1".into(),
                    event_id: "audio:1".into(),
                    labels: vec!["GraphNode".into(), "Sensation".into()],
                    text: "audio sensation; transcript: hello".into(),
                    occurred_at: "2026-05-05T12:34:56Z".into(),
                },
                GraphTimelineItem {
                    id: "sensation:thought:1".into(),
                    event_id: "combobulation-summary:1".into(),
                    labels: vec!["GraphNode".into(), "Sensation".into()],
                    text: "combobulation sensation; I may be hearing a greeting.".into(),
                    occurred_at: "2026-05-05T12:34:57Z".into(),
                },
            ],
        };

        let ts1 = timeline_timestamp("2026-05-05T12:34:56Z");
        let ts2 = timeline_timestamp("2026-05-05T12:34:57Z");
        assert_eq!(
            timeline_prompt(&window),
            format!(
                "Sensation timeline {ts1} to {ts2}\n[{ts1}] audio sensation; transcript: hello\n[{ts2}] combobulation sensation; I may be hearing a greeting."
            )
        );
    }
}

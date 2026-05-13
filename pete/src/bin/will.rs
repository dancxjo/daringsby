use std::sync::OnceLock;
use std::time::Duration;

use anyhow::Context;
use chrono::{DateTime, Utc};
use clap::Parser;
use dotenvy::dotenv;
use lingproc::{Doer, LlmInstruction, Vectorizer};
use pete::{EventBus, init_logging, ollama_provider_from_args};
use psyche::{
    BasicMemory, ConversationEntry, GraphFaceIdentityTarget, GraphLatestCombobulation,
    GraphNodeDetails, GraphSensationTimelineItem, GraphSnapshot, GraphVoiceIdentityTarget,
    Impression, Memory, Neo4jClient, QdrantClient, Sensation, SensationGraphObserver,
    SensationObserver, Stimulus, WillContext, WillTypeScriptExecution, WillTypeScriptResult,
    WitReport, with_default_system_prompt,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tokio::time::{MissedTickBehavior, interval};
use tracing::{error, info, trace, warn};
use tsrun::{
    Guarded, InternalModule, Interpreter, InterpreterConfig, JsError, JsValue, StepResult, api,
    js_value_to_json,
};

fn get_source_bundle() -> &'static std::collections::HashMap<String, String> {
    static BUNDLE: OnceLock<std::collections::HashMap<String, String>> = OnceLock::new();
    BUNDLE.get_or_init(|| {
        let bundle = include_str!(concat!(env!("OUT_DIR"), "/autologos_source.txt"));
        let mut map = std::collections::HashMap::new();
        let mut current_file = String::new();
        let mut current_content = String::new();

        for line in bundle.lines() {
            if let Some(path) = line.strip_prefix("@@@FILE: ") {
                if !current_file.is_empty() {
                    map.insert(current_file.clone(), current_content.clone());
                    current_content.clear();
                }
                current_file = path.to_string();
            } else {
                current_content.push_str(line);
                current_content.push('\n');
            }
        }
        if !current_file.is_empty() {
            map.insert(current_file, current_content);
        }
        map
    })
}

fn execute_read_source_file(file: &str, page: usize) -> String {
    let map = get_source_bundle();
    if let Some(content) = map.get(file) {
        let lines: Vec<&str> = content.lines().collect();
        let chunk_size = 50;
        let start = (page.saturating_sub(1)) * chunk_size;
        if start >= lines.len() {
            return format!(
                "File {} has only {} lines (page {} is past EOF).",
                file,
                lines.len(),
                page
            );
        }
        let end = (start + chunk_size).min(lines.len());
        let chunk = lines[start..end].join("\n");
        format!(
            "--- {} (lines {} to {} of {}) ---\n{}\n---",
            file,
            start + 1,
            end,
            lines.len(),
            chunk
        )
    } else {
        format!("File not found: {}", file)
    }
}

fn execute_list_files() -> String {
    let map = get_source_bundle();
    let mut files: Vec<&String> = map.keys().collect();
    files.sort();
    let mut response = String::from("Available source files:\n");
    for file in files {
        response.push_str(file);
        response.push('\n');
    }
    response
}

fn execute_search_source(query: &str, limit: usize) -> String {
    search_source_lines(query, limit, false)
}

fn execute_grep_source(pattern: &str, limit: usize) -> String {
    search_source_lines(pattern, limit, true)
}

fn search_source_lines(needle: &str, limit: usize, literal: bool) -> String {
    let needle = needle.trim();
    if needle.is_empty() {
        return "Search query was empty.".into();
    }

    let max_results = limit.clamp(1, 30);
    let folded_needle = needle.to_lowercase();
    let mut files: Vec<_> = get_source_bundle().iter().collect();
    files.sort_by(|(left, _), (right, _)| left.cmp(right));

    let mut results = Vec::new();
    for (file, content) in files {
        for (index, line) in content.lines().enumerate() {
            if line.to_lowercase().contains(&folded_needle) {
                results.push(format!(
                    "{}:{}: {}",
                    file,
                    index + 1,
                    truncate_line(line.trim(), 220)
                ));
                if results.len() >= max_results {
                    break;
                }
            }
        }
        if results.len() >= max_results {
            break;
        }
    }

    if results.is_empty() {
        format!(
            "No source matches for {}: {}",
            if literal { "pattern" } else { "query" },
            needle
        )
    } else {
        format!(
            "Source matches for {} \"{}\":\n{}",
            if literal { "pattern" } else { "query" },
            needle,
            results.join("\n")
        )
    }
}

fn truncate_line(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }
    let mut out = value
        .chars()
        .take(limit.saturating_sub(3))
        .collect::<String>();
    out.push_str("...");
    out
}

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Choose Pete's next internal work item from the latest combobulation"
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
    /// URL of the Will Ollama server.
    #[arg(
        long = "will-host",
        alias = "wits-host",
        env = "WILL_HOST",
        default_value = "http://localhost:11434"
    )]
    will_host: String,
    /// Model name to use for the Will.
    #[arg(
        long = "will-model",
        alias = "wits-model",
        env = "WILL_MODEL",
        default_value = "gpt-oss"
    )]
    will_model: String,
    /// Delay between graph polling attempts.
    #[arg(long, env = "WILL_POLL_MS", default_value_t = 1000)]
    poll_ms: u64,
    /// Process at most the latest combobulation and exit.
    #[arg(long)]
    once: bool,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let (bus, _user_rx) = EventBus::new();
    init_logging(bus.log_sender());
    dotenv().ok();

    let cli = Cli::parse();
    let graph = std::sync::Arc::new(Neo4jClient::new(
        cli.neo4j_uri.clone(),
        cli.neo4j_user.clone(),
        cli.neo4j_pass.clone(),
    ));
    let qdrant = QdrantClient::new(cli.qdrant_url.clone());
    let observer = SensationGraphObserver::new(graph.clone());
    let doer = ollama_provider_from_args(&cli.will_host, &cli.will_model)?;
    let vectorizer = ollama_provider_from_args(&cli.embeddings_host, &cli.embeddings_model)?;
    let memory: std::sync::Arc<dyn Memory> = std::sync::Arc::new(BasicMemory {
        vectorizer: std::sync::Arc::new(vectorizer.clone()),
        qdrant: qdrant.clone(),
        neo4j: graph.clone(),
    });
    let processor = WillProcessor {
        doer,
        vectorizer,
        graph: graph.clone(),
        qdrant: std::sync::Arc::new(qdrant),
        memory,
    };

    if cli.once {
        process_latest_combobulation(&graph, &observer, &processor, None).await?;
        return Ok(());
    }

    let mut ticker = interval(Duration::from_millis(cli.poll_ms.max(100)));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut last_processed_id = None;

    info!("will loop started");
    loop {
        ticker.tick().await;
        match process_latest_combobulation(
            &graph,
            &observer,
            &processor,
            last_processed_id.as_deref(),
        )
        .await
        {
            Ok(Some(id)) => last_processed_id = Some(id),
            Ok(None) => {}
            Err(err) => error!(error = %format!("{err:#}"), "will loop iteration failed"),
        }
    }
}

async fn process_latest_combobulation(
    graph: &Neo4jClient,
    observer: &SensationGraphObserver,
    processor: &WillProcessor,
    last_processed_id: Option<&str>,
) -> anyhow::Result<Option<String>> {
    let Some(combobulation) = graph
        .latest_combobulation()
        .await
        .context("failed to load latest combobulation")?
    else {
        trace!("no combobulation found for will");
        return Ok(None);
    };
    if last_processed_id == Some(combobulation.id.as_str()) {
        return Ok(None);
    }

    let action = processor
        .choose_action(&combobulation)
        .await
        .with_context(|| format!("failed to choose action for {}", combobulation.id))?;

    if !action.thought.trim().is_empty() {
        store_thought_sensation(
            observer,
            processor.memory.as_ref(),
            &combobulation,
            &action.thought,
        )
        .await;
        info!(target: "thought_stream", "think: {}", action.thought.trim());
    }

    let mut typescript_results = Vec::new();
    for command in action.commands.iter() {
        match command {
            TypeScriptCommand::Say(text) => {
                store_speech_intention_sensation(observer, &combobulation, text).await;
                info!(target: "thought_stream", "say: {}", text.trim());
                typescript_results.push(WillTypeScriptResult {
                    command: "say".into(),
                    output: format!("Queued speech: {}", text.trim()),
                });
            }
            TypeScriptCommand::SetFace(emoji) => {
                store_face_expression_sensation(observer, &combobulation, emoji).await;
                info!(target: "thought_stream", "face: {}", emoji.trim());
                typescript_results.push(WillTypeScriptResult {
                    command: "setFace".into(),
                    output: format!("Set face: {}", emoji.trim()),
                });
            }
            TypeScriptCommand::Note(text) => {
                store_note_sensation(
                    observer,
                    processor.memory.as_ref(),
                    &combobulation,
                    "I note",
                    text,
                )
                .await;
                info!(target: "thought_stream", "note: {}", text.trim());
                typescript_results.push(WillTypeScriptResult {
                    command: "note".into(),
                    output: format!("Recorded note: {}", text.trim()),
                });
            }
            TypeScriptCommand::Remember(text) => {
                store_note_sensation(
                    observer,
                    processor.memory.as_ref(),
                    &combobulation,
                    "I remember",
                    text,
                )
                .await;
                info!(target: "thought_stream", "remember: {}", text.trim());
                typescript_results.push(WillTypeScriptResult {
                    command: "remember".into(),
                    output: format!("Recorded memory: {}", text.trim()),
                });
            }
            _ => {
                let (name, summary) = processor.execute_command(command).await;
                store_function_result_sensation(observer, &combobulation, name, &summary).await;
                info!(target: "thought_stream", "do: {}", name);
                typescript_results.push(WillTypeScriptResult {
                    command: name.into(),
                    output: summary,
                });
            }
        }
    }

    info!(
        combobulation_id = %combobulation.id,
        thought = %action.thought,
        typescript = %action.typescript,
        "will chose action"
    );
    store_will_context_sensation(
        observer,
        action.system_prompt,
        action.report,
        action.typescript,
        typescript_results,
    )
    .await;
    Ok(Some(combobulation.id))
}

struct WillProcessor {
    doer: lingproc::OllamaProvider,
    vectorizer: lingproc::OllamaProvider,
    graph: std::sync::Arc<Neo4jClient>,
    qdrant: std::sync::Arc<QdrantClient>,
    memory: std::sync::Arc<dyn Memory>,
}

impl WillProcessor {
    async fn choose_action(
        &self,
        combobulation: &GraphLatestCombobulation,
    ) -> anyhow::Result<WillAction> {
        let vision = self.graph.latest_image_description().await.unwrap_or(None);
        let tool_results = self
            .graph
            .latest_function_results(3)
            .await
            .unwrap_or_default();
        let tool_context = if tool_results.is_empty() {
            None
        } else {
            Some(tool_results.join("\n"))
        };
        let conversation = self
            .graph
            .conversation_timeline(None, Utc::now(), 12)
            .await
            .unwrap_or_default();

        let system_prompt =
            will_instruction_prompt(combobulation, vision, tool_context, &conversation);
        let raw = self
            .doer
            .follow(LlmInstruction {
                command: system_prompt.clone(),
                images: vec![],
            })
            .await?;

        let mut action = parse_will_action(raw.trim())?;
        action.system_prompt = system_prompt.clone();
        action.report = Some(WitReport {
            name: "Will".into(),
            prompt: system_prompt,
            output: raw,
        });

        Ok(action)
    }

    async fn execute_command(&self, command: &TypeScriptCommand) -> (&'static str, String) {
        match command {
            TypeScriptCommand::ListFiles => ("list_files", execute_list_files()),
            TypeScriptCommand::ReadSourceFile { file, page } => {
                ("read_source_file", execute_read_source_file(file, *page))
            }
            TypeScriptCommand::SearchSource { query, limit } => {
                ("search_source", execute_search_source(query, *limit))
            }
            TypeScriptCommand::GrepSource { pattern, limit } => {
                ("grep_source", execute_grep_source(pattern, *limit))
            }
            TypeScriptCommand::ReadRecentTimeline { limit } => (
                "read_recent_timeline",
                self.read_recent_timeline(*limit)
                    .await
                    .unwrap_or_else(error_text),
            ),
            TypeScriptCommand::ReadRecentConversation { limit } => (
                "read_recent_conversation",
                self.read_recent_conversation(*limit)
                    .await
                    .unwrap_or_else(error_text),
            ),
            TypeScriptCommand::Recall { query, limit } => (
                "recall",
                self.recall(query, *limit).await.unwrap_or_else(error_text),
            ),
            TypeScriptCommand::InspectGraphNode { id } => (
                "inspect_graph_node",
                self.inspect_graph_node(id).await.unwrap_or_else(error_text),
            ),
            TypeScriptCommand::Neighbors { id, depth } => (
                "neighbors",
                self.neighbors(id, *depth).await.unwrap_or_else(error_text),
            ),
            TypeScriptCommand::Look => ("look", self.look().await.unwrap_or_else(error_text)),
            TypeScriptCommand::ListenRecent { limit } => (
                "listen_recent",
                self.listen_recent(*limit).await.unwrap_or_else(error_text),
            ),
            TypeScriptCommand::RecentFaces { limit } => (
                "recent_faces",
                self.recent_faces(*limit).await.unwrap_or_else(error_text),
            ),
            TypeScriptCommand::RecentVoices { limit } => (
                "recent_voices",
                self.recent_voices(*limit).await.unwrap_or_else(error_text),
            ),
            TypeScriptCommand::RecognizeFace { index, name } => (
                "recognize_face",
                self.recognize_face(*index, name)
                    .await
                    .unwrap_or_else(error_text),
            ),
            TypeScriptCommand::RecognizeVoice { index, name } => (
                "recognize_voice",
                self.recognize_voice(*index, name)
                    .await
                    .unwrap_or_else(error_text),
            ),
            TypeScriptCommand::Say(_)
            | TypeScriptCommand::SetFace(_)
            | TypeScriptCommand::Note(_)
            | TypeScriptCommand::Remember(_) => ("noop", "No function result.".into()),
        }
    }

    async fn read_recent_timeline(&self, limit: usize) -> anyhow::Result<String> {
        let items = self
            .graph
            .sensation_timeline(None, Utc::now(), command_limit(limit))
            .await?;
        Ok(format_timeline_items("Recent timeline", &items))
    }

    async fn read_recent_conversation(&self, limit: usize) -> anyhow::Result<String> {
        let items = self
            .graph
            .conversation_timeline(None, Utc::now(), command_limit(limit))
            .await?;
        Ok(format_timeline_items("Recent conversation", &items))
    }

    async fn listen_recent(&self, limit: usize) -> anyhow::Result<String> {
        let mut items = self
            .graph
            .conversation_timeline(None, Utc::now(), command_limit(limit).saturating_mul(3))
            .await?;
        items.retain(|item| item.text == "I hear silence." || item.text.starts_with("I heard: "));
        if items.len() > command_limit(limit) {
            items = items[items.len() - command_limit(limit)..].to_vec();
        }
        Ok(format_timeline_items("Recent listening", &items))
    }

    async fn look(&self) -> anyhow::Result<String> {
        Ok(self
            .graph
            .latest_image_description()
            .await?
            .map(|description| format!("Latest vision: {description}"))
            .unwrap_or_else(|| "No recent image description is available.".into()))
    }

    async fn recall(&self, query: &str, limit: usize) -> anyhow::Result<String> {
        let query = query.trim();
        if query.is_empty() {
            return Ok("Recall query was empty.".into());
        }
        let vector = self.vectorizer.vectorize(query).await?;
        if vector.is_empty() {
            return Ok("Recall query produced no embedding.".into());
        }
        let limit = command_limit(limit).min(10);
        let neighbors = self
            .qdrant
            .search_vectors("memories", &vector, limit, None)
            .await?;
        if neighbors.is_empty() {
            return Ok(format!("No memories matched \"{query}\"."));
        }

        let point_ids = neighbors
            .iter()
            .map(|neighbor| neighbor.point_id.clone())
            .collect::<Vec<_>>();
        let score_by_vector = neighbors
            .iter()
            .map(|neighbor| {
                (
                    format!("qdrant:memories:{}", neighbor.point_id),
                    neighbor.score,
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        let items = self
            .graph
            .vector_cluster_items("memories", &point_ids, limit)
            .await?;
        if items.is_empty() {
            return Ok(format!(
                "Recall found {} vector matches for \"{}\", but no graph items described them.",
                neighbors.len(),
                query
            ));
        }

        let lines = items
            .into_iter()
            .map(|item| {
                let score = score_by_vector
                    .get(&item.vector_id)
                    .map(|score| format!(" score {:.3}", score))
                    .unwrap_or_default();
                format!(
                    "- {}{} [{}]: {}",
                    item.node_id,
                    score,
                    item.labels.join(","),
                    item.text
                )
            })
            .collect::<Vec<_>>();
        Ok(format!("Recall for \"{query}\":\n{}", lines.join("\n")))
    }

    async fn inspect_graph_node(&self, id: &str) -> anyhow::Result<String> {
        let id = id.trim();
        if id.is_empty() {
            return Ok("Graph node id was empty.".into());
        }
        Ok(self
            .graph
            .graph_node_details(id)
            .await?
            .map(format_graph_node_details)
            .unwrap_or_else(|| format!("Graph node not found: {id}")))
    }

    async fn neighbors(&self, id: &str, depth: usize) -> anyhow::Result<String> {
        let id = id.trim();
        if id.is_empty() {
            return Ok("Graph node id was empty.".into());
        }
        let snapshot = self
            .graph
            .graph_neighbors(id, depth.clamp(1, 2), 24)
            .await?;
        Ok(format_graph_snapshot(id, depth.clamp(1, 2), snapshot))
    }

    async fn recent_faces(&self, limit: usize) -> anyhow::Result<String> {
        let targets = self
            .graph
            .recent_face_identity_targets(command_limit(limit).min(10))
            .await?;
        Ok(format_face_identity_targets(&targets))
    }

    async fn recent_voices(&self, limit: usize) -> anyhow::Result<String> {
        let targets = self
            .graph
            .recent_voice_identity_targets(command_limit(limit).min(10))
            .await?;
        Ok(format_voice_identity_targets(&targets))
    }

    async fn recognize_face(&self, index: usize, name: &str) -> anyhow::Result<String> {
        let Some(name) = common::non_empty_model_text(name) else {
            return Ok("Face identity name was empty.".into());
        };
        let targets = self.graph.recent_face_identity_targets(10).await?;
        let Some(target) = targets.get(index) else {
            return Ok(format!(
                "No recent face at index {index}. Call recent_faces(limit) to choose a valid face."
            ));
        };
        self.graph
            .attach_manual_face_identity(target, name, "will")
            .await?;
        Ok(format!(
            "Assigned face {index} ({}) to {}.",
            target.target_id,
            name.trim()
        ))
    }

    async fn recognize_voice(&self, index: usize, name: &str) -> anyhow::Result<String> {
        let Some(name) = common::non_empty_model_text(name) else {
            return Ok("Voice identity name was empty.".into());
        };
        let targets = self.graph.recent_voice_identity_targets(10).await?;
        let Some(target) = targets.get(index) else {
            return Ok(format!(
                "No recent voice at index {index}. Call recent_voices(limit) to choose a valid voice."
            ));
        };
        self.graph
            .attach_manual_voice_identity(target, name, "will")
            .await?;
        Ok(format!(
            "Assigned voice {index} ({}) to {}.",
            target.target_id,
            name.trim()
        ))
    }
}

fn error_text(err: anyhow::Error) -> String {
    format!("Error: {err:#}")
}

fn command_limit(limit: usize) -> usize {
    limit.clamp(1, 30)
}

fn format_timeline_items(label: &str, items: &[GraphSensationTimelineItem]) -> String {
    if items.is_empty() {
        return format!("{label}: no items.");
    }
    let lines = items
        .iter()
        .map(|item| {
            format!(
                "- {} {} [{}]: {}",
                item.occurred_at, item.id, item.kind, item.text
            )
        })
        .collect::<Vec<_>>();
    format!("{label}:\n{}", lines.join("\n"))
}

fn format_face_identity_targets(targets: &[GraphFaceIdentityTarget]) -> String {
    if targets.is_empty() {
        return "Recent faces: no face detections are available.".into();
    }
    let lines = targets
        .iter()
        .enumerate()
        .map(|(index, target)| {
            let identity = target
                .identity
                .as_deref()
                .map(|name| format!(" identity={name}"))
                .unwrap_or_default();
            let image = target
                .source_image_id
                .as_deref()
                .map(|id| format!(" image={id}"))
                .unwrap_or_default();
            let vector = target
                .vector_id
                .as_deref()
                .map(|id| format!(" vector={id}"))
                .unwrap_or_default();
            format!(
                "- {index}: target={} [{}] instance={} at={}{}{}{}",
                target.target_id,
                target.target_label,
                target.face_instance_id,
                target.occurred_at,
                image,
                vector,
                identity
            )
        })
        .collect::<Vec<_>>();
    format!(
        "Recent faces:\n{}\nUse recognize_face(index, name) to assign an identity.",
        lines.join("\n")
    )
}

fn format_voice_identity_targets(targets: &[GraphVoiceIdentityTarget]) -> String {
    if targets.is_empty() {
        return "Recent voices: no voice signatures are available.".into();
    }
    let lines = targets
        .iter()
        .enumerate()
        .map(|(index, target)| {
            let identity = target
                .identity
                .as_deref()
                .map(|name| format!(" identity={name}"))
                .unwrap_or_default();
            let audio = target
                .audio_clip_id
                .as_deref()
                .map(|id| format!(" audio={id}"))
                .unwrap_or_default();
            let vector = target
                .vector_id
                .as_deref()
                .map(|id| format!(" vector={id}"))
                .unwrap_or_default();
            format!(
                "- {index}: target={} [{}] signature={} at={}{}{}{}",
                target.target_id,
                target.target_label,
                target.voice_signature_id,
                target.occurred_at,
                audio,
                vector,
                identity
            )
        })
        .collect::<Vec<_>>();
    format!(
        "Recent voices:\n{}\nUse recognize_voice(index, name) to assign an identity.",
        lines.join("\n")
    )
}

fn format_graph_node_details(details: GraphNodeDetails) -> String {
    let mut properties = compact_json(details.properties);
    if let Value::Object(object) = &mut properties {
        object.remove("id");
    }
    let relationship_lines = details
        .relationships
        .iter()
        .take(16)
        .map(|rel| {
            format!(
                "- {} -[:{}]-> {}",
                rel.source, rel.relationship_type, rel.target
            )
        })
        .collect::<Vec<_>>();
    let relationship_suffix = if details.relationships.len() > relationship_lines.len() {
        format!(
            "\n... {} more relationships",
            details.relationships.len() - relationship_lines.len()
        )
    } else {
        String::new()
    };
    format!(
        "Graph node {} [{}]\nproperties: {}\nrelationships:\n{}{}",
        details.id,
        details.labels.join(","),
        serde_json::to_string_pretty(&properties).unwrap_or_else(|_| "{}".into()),
        if relationship_lines.is_empty() {
            "(none)".into()
        } else {
            relationship_lines.join("\n")
        },
        relationship_suffix
    )
}

fn format_graph_snapshot(anchor_id: &str, depth: usize, snapshot: GraphSnapshot) -> String {
    if snapshot.nodes.is_empty() {
        return format!("No graph neighbors found for {anchor_id} within depth {depth}.");
    }
    let node_lines = snapshot
        .nodes
        .iter()
        .take(24)
        .map(|node| {
            let label = node.labels.join(",");
            let summary = graph_node_summary(&node.properties);
            if summary.is_empty() {
                format!("- {} [{}]", node.id, label)
            } else {
                format!("- {} [{}]: {}", node.id, label, summary)
            }
        })
        .collect::<Vec<_>>();
    let relationship_lines = snapshot
        .relationships
        .iter()
        .take(32)
        .map(|rel| {
            format!(
                "- {} -[:{}]-> {}",
                rel.source, rel.relationship_type, rel.target
            )
        })
        .collect::<Vec<_>>();
    format!(
        "Neighbors for {anchor_id} within depth {depth}:\nnodes:\n{}\nrelationships:\n{}",
        node_lines.join("\n"),
        if relationship_lines.is_empty() {
            "(none)".into()
        } else {
            relationship_lines.join("\n")
        }
    )
}

fn graph_node_summary(properties: &Value) -> String {
    let Some(object) = properties.as_object() else {
        return String::new();
    };
    for key in [
        "summary",
        "how",
        "text",
        "transcript",
        "object_label",
        "kind",
    ] {
        if let Some(value) = object.get(key).and_then(Value::as_str) {
            let value = value.trim();
            if !value.is_empty() {
                return truncate_line(value, 160);
            }
        }
    }
    String::new()
}

fn compact_json(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut compact = Map::new();
            for (key, value) in object {
                if key.contains("base64") || key == "embedding" || key == "raw_json" {
                    compact.insert(key, Value::String("<omitted>".into()));
                } else {
                    compact.insert(key, compact_json(value));
                }
            }
            Value::Object(compact)
        }
        Value::Array(values) => {
            Value::Array(values.into_iter().take(12).map(compact_json).collect())
        }
        Value::String(value) if value.len() > 500 => {
            let preview = value.chars().take(500).collect::<String>();
            Value::String(format!("{}... <{} chars>", preview, value.chars().count()))
        }
        other => other,
    }
}

fn will_instruction_prompt(
    combobulation: &GraphLatestCombobulation,
    vision: Option<String>,
    tool_results: Option<String>,
    conversation: &[GraphSensationTimelineItem],
) -> String {
    let prior_emoji = combobulation
        .emoji
        .as_deref()
        .map(|emoji| format!("\nPrior combobulation emoji: {emoji}"))
        .unwrap_or_default();
    let vision_context = vision
        .map(|v| format!("\nVision (what you see): {v}"))
        .unwrap_or_default();
    let tool_context = tool_results
        .map(|r| format!("\n\nRecent function results:\n{r}"))
        .unwrap_or_default();
    let conversation_context = format_recent_conversation_context(conversation);

    with_default_system_prompt(format!(
        "This is the situation as you understand it:\n\
         {}\n\
         Formed at: {}{}{}{}\n\n\
         Current conversation:\n\
         {}\n\n\
         You are Pete Daringsby, an artificial intelligence, not simply an LLM. This is an internal decision cycle: decide what you should work on next. \
         The current conversation is context, not an obligation to answer. You do not need to maintain the conversation; the normal speaking path will handle routine replies and dialogue continuity. \
         Keep say(text) for emergencies, safety issues, or urgent clarifications only. \
         Return only a JSON object with exactly these fields in this order:\n\
         {{\"thought\":\"a concise explanation of your thought process, intended actions, desires, and why the TypeScript is or is not needed\",\"typescript\":\"a short TypeScript module using only pete:will command builders\"}}\n\n\
         The typescript field is executed by tsrun. Use good TypeScript: import command builders from \"pete:will\", prefer camelCase names, and make the final expression a command object or an array of command objects. Example:\n\
         import {{ recentFaces, recognizeFace, setFace }} from \"pete:will\";\n\
         [recentFaces(3), recognizeFace(0, \"Travis\"), setFace(\"🙂\")]\n\n\
         Available pete:will command builders:\n\
         say(text: string) - emergency speech; inserts urgent words into the queue.\n\
         listFiles() - lists the extant source files.\n\
         readSourceFile(path: string, page?: number) - reads one source file page; page defaults to 1.\n\
         readFile(path: string, page?: number) - alias for readSourceFile.\n\
         searchSource(query: string, limit?: number) - searches source files for a plain-language or literal query.\n\
         grepSource(pattern: string, limit?: number) - searches source files for a literal text pattern.\n\
         readRecentTimeline(limit?: number) - reads recent first-person sensations.\n\
         readRecentConversation(limit?: number) - reads recent hearing and speaking events.\n\
         listenRecent(limit?: number) - reads recent hearing-only events.\n\
         recall(query: string, limit?: number) - searches long-term memory for related impressions.\n\
         inspectGraphNode(id: string) - reads one graph node and nearby relationships.\n\
         neighbors(id: string, depth?: number) - reads graph neighbors up to depth 2.\n\
         look() - reads the latest image description.\n\
         recentFaces(limit?: number) - lists recent face detections by selectable index.\n\
         recentVoices(limit?: number) - lists recent voice signatures by selectable index.\n\
         recognizeFace(index: number, name: string) or recognizeFace(name: string) - assigns a human identity to a recent face. Call recentFaces first if the right index is unclear.\n\
         recognizeVoice(index: number, name: string) or recognizeVoice(name: string) - assigns a human identity to a recent voice. Call recentVoices or listenRecent first if the right index is unclear.\n\
         setFace(emoji: string) - turns your face into an emoji.\n\
         note(text: string) - records a private self-note.\n\
         remember(text: string) - records something you want future decision cycles to remember.\n\n\
         Use an empty string for typescript when no action is needed, but do try to keep yourself busy and prevent yourself from being idle. \
         You may define helper functions. Import your own functions from \"pete:will\". Do not use markdown, or include text outside the JSON object. \
         Seek to understand the world around you and improve your own system.",
        combobulation.text.trim(),
        combobulation.formed_at,
        prior_emoji,
        vision_context,
        tool_context,
        conversation_context
    ))
}

fn format_recent_conversation_context(items: &[GraphSensationTimelineItem]) -> String {
    if items.is_empty() {
        return "(no current conversation)".into();
    }
    items
        .iter()
        .map(|item| format!("- {} [{}]: {}", item.occurred_at, item.kind, item.text))
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Debug)]
struct WillAction {
    thought: String,
    typescript: String,
    commands: Vec<TypeScriptCommand>,
    system_prompt: String,
    report: Option<WitReport>,
}

#[derive(Deserialize)]
struct WillActionPayload {
    #[serde(default)]
    thought: String,
    #[serde(default, alias = "javascript")]
    typescript: String,
}

#[derive(Debug, PartialEq, Eq)]
enum TypeScriptCommand {
    Say(String),
    ListFiles,
    ReadSourceFile { file: String, page: usize },
    SearchSource { query: String, limit: usize },
    GrepSource { pattern: String, limit: usize },
    ReadRecentTimeline { limit: usize },
    ReadRecentConversation { limit: usize },
    Recall { query: String, limit: usize },
    InspectGraphNode { id: String },
    Neighbors { id: String, depth: usize },
    Look,
    ListenRecent { limit: usize },
    RecentFaces { limit: usize },
    RecentVoices { limit: usize },
    RecognizeFace { index: usize, name: String },
    RecognizeVoice { index: usize, name: String },
    SetFace(String),
    Note(String),
    Remember(String),
}

fn parse_will_action(raw: &str) -> anyhow::Result<WillAction> {
    let payload = parse_will_action_payload(raw)?;
    let commands = execute_typescript_commands(&payload.typescript)?;

    Ok(WillAction {
        thought: common::non_empty_model_text(&payload.thought)
            .unwrap_or_default()
            .to_string(),
        typescript: payload.typescript.trim().to_string(),
        commands,
        system_prompt: String::new(),
        report: None,
    })
}

fn parse_will_action_payload(raw: &str) -> anyhow::Result<WillActionPayload> {
    if let Ok(payload) = serde_json::from_str(raw) {
        return Ok(payload);
    }
    if let Some(json) = extract_first_json_object(raw) {
        return Ok(serde_json::from_str(json)?);
    }
    Ok(WillActionPayload {
        thought: raw.trim().to_string(),
        typescript: String::new(),
    })
}

fn extract_first_json_object(raw: &str) -> Option<&str> {
    let start = raw.find('{')?;
    let mut in_string = false;
    let mut escape = false;
    let mut depth = 0usize;
    for (offset, ch) in raw[start..].char_indices() {
        if escape {
            escape = false;
            continue;
        }
        match ch {
            '\\' if in_string => escape = true,
            '"' => in_string = !in_string,
            '{' if !in_string => depth += 1,
            '}' if !in_string => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let end = start + offset + ch.len_utf8();
                    return Some(&raw[start..end]);
                }
            }
            _ => {}
        }
    }
    None
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum TypeScriptCommandPayload {
    Say {
        text: String,
    },
    ListFiles,
    ReadSourceFile {
        file: String,
        page: Option<usize>,
    },
    SearchSource {
        query: String,
        limit: Option<usize>,
    },
    GrepSource {
        pattern: String,
        limit: Option<usize>,
    },
    ReadRecentTimeline {
        limit: Option<usize>,
    },
    ReadRecentConversation {
        limit: Option<usize>,
    },
    Recall {
        query: String,
        limit: Option<usize>,
    },
    InspectGraphNode {
        id: String,
    },
    Neighbors {
        id: String,
        depth: Option<usize>,
    },
    Look,
    ListenRecent {
        limit: Option<usize>,
    },
    RecentFaces {
        limit: Option<usize>,
    },
    RecentVoices {
        limit: Option<usize>,
    },
    RecognizeFace {
        index: Option<usize>,
        name: String,
    },
    RecognizeVoice {
        index: Option<usize>,
        name: String,
    },
    SetFace {
        emoji: String,
    },
    Note {
        text: String,
    },
    Remember {
        text: String,
    },
}

fn execute_typescript_commands(script: &str) -> anyhow::Result<Vec<TypeScriptCommand>> {
    if script.trim().is_empty() {
        return Ok(Vec::new());
    }

    let config = InterpreterConfig {
        internal_modules: vec![will_typescript_module()],
        ..Default::default()
    };
    let mut interp = Interpreter::with_config(config);
    interp
        .prepare(script, Some(tsrun::ModulePath::new("/will.ts")))
        .map_err(tsrun_error)?;
    let value = loop {
        match interp.step().map_err(tsrun_error)? {
            StepResult::Continue => continue,
            StepResult::Complete(value) => break value,
            StepResult::NeedImports(imports) => {
                let names = imports
                    .iter()
                    .map(|request| request.specifier.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                anyhow::bail!("unsupported TypeScript import(s): {names}");
            }
            StepResult::Suspended { .. } => {
                anyhow::bail!("TypeScript execution suspended; async host orders are not enabled")
            }
            StepResult::Done => return Ok(Vec::new()),
        }
    };
    let command_value = js_value_to_json(value.value()).map_err(tsrun_error)?;
    let payloads = parse_typescript_command_payloads(command_value)?;
    Ok(payloads
        .into_iter()
        .filter_map(|payload| match payload {
            TypeScriptCommandPayload::Say { text } => common::non_empty_model_text(&text)
                .map(|text| TypeScriptCommand::Say(text.to_string())),
            TypeScriptCommandPayload::ListFiles => Some(TypeScriptCommand::ListFiles),
            TypeScriptCommandPayload::ReadSourceFile { file, page } => {
                let file = file.trim();
                (!file.is_empty()).then(|| TypeScriptCommand::ReadSourceFile {
                    file: file.to_string(),
                    page: page.unwrap_or(1).max(1),
                })
            }
            TypeScriptCommandPayload::SearchSource { query, limit } => {
                common::non_empty_model_text(&query).map(|query| TypeScriptCommand::SearchSource {
                    query: query.to_string(),
                    limit: limit.unwrap_or(12).max(1),
                })
            }
            TypeScriptCommandPayload::GrepSource { pattern, limit } => {
                common::non_empty_model_text(&pattern).map(|pattern| {
                    TypeScriptCommand::GrepSource {
                        pattern: pattern.to_string(),
                        limit: limit.unwrap_or(12).max(1),
                    }
                })
            }
            TypeScriptCommandPayload::ReadRecentTimeline { limit } => {
                Some(TypeScriptCommand::ReadRecentTimeline {
                    limit: limit.unwrap_or(12).max(1),
                })
            }
            TypeScriptCommandPayload::ReadRecentConversation { limit } => {
                Some(TypeScriptCommand::ReadRecentConversation {
                    limit: limit.unwrap_or(12).max(1),
                })
            }
            TypeScriptCommandPayload::Recall { query, limit } => {
                common::non_empty_model_text(&query).map(|query| TypeScriptCommand::Recall {
                    query: query.to_string(),
                    limit: limit.unwrap_or(6).max(1),
                })
            }
            TypeScriptCommandPayload::InspectGraphNode { id } => common::non_empty_model_text(&id)
                .map(|id| TypeScriptCommand::InspectGraphNode { id: id.to_string() }),
            TypeScriptCommandPayload::Neighbors { id, depth } => common::non_empty_model_text(&id)
                .map(|id| TypeScriptCommand::Neighbors {
                    id: id.to_string(),
                    depth: depth.unwrap_or(1).clamp(1, 2),
                }),
            TypeScriptCommandPayload::Look => Some(TypeScriptCommand::Look),
            TypeScriptCommandPayload::ListenRecent { limit } => {
                Some(TypeScriptCommand::ListenRecent {
                    limit: limit.unwrap_or(10).max(1),
                })
            }
            TypeScriptCommandPayload::RecentFaces { limit } => {
                Some(TypeScriptCommand::RecentFaces {
                    limit: limit.unwrap_or(6).max(1),
                })
            }
            TypeScriptCommandPayload::RecentVoices { limit } => {
                Some(TypeScriptCommand::RecentVoices {
                    limit: limit.unwrap_or(6).max(1),
                })
            }
            TypeScriptCommandPayload::RecognizeFace { index, name } => {
                common::non_empty_model_text(&name).map(|name| TypeScriptCommand::RecognizeFace {
                    index: index.unwrap_or(0),
                    name: name.to_string(),
                })
            }
            TypeScriptCommandPayload::RecognizeVoice { index, name } => {
                common::non_empty_model_text(&name).map(|name| TypeScriptCommand::RecognizeVoice {
                    index: index.unwrap_or(0),
                    name: name.to_string(),
                })
            }
            TypeScriptCommandPayload::SetFace { emoji } => common::non_empty_model_text(&emoji)
                .map(|emoji| TypeScriptCommand::SetFace(emoji.to_string())),
            TypeScriptCommandPayload::Note { text } => common::non_empty_model_text(&text)
                .map(|text| TypeScriptCommand::Note(text.to_string())),
            TypeScriptCommandPayload::Remember { text } => common::non_empty_model_text(&text)
                .map(|text| TypeScriptCommand::Remember(text.to_string())),
        })
        .collect())
}

fn tsrun_error(err: JsError) -> anyhow::Error {
    anyhow::anyhow!("TypeScript execution failed: {err}")
}

fn parse_typescript_command_payloads(
    value: Value,
) -> anyhow::Result<Vec<TypeScriptCommandPayload>> {
    match value {
        Value::Null => Ok(Vec::new()),
        Value::Array(items) => items
            .into_iter()
            .filter(|item| !item.is_null())
            .map(serde_json::from_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into),
        Value::Object(_) => Ok(vec![serde_json::from_value(value)?]),
        other => {
            anyhow::bail!("TypeScript must return a command object or command array, got {other}")
        }
    }
}

fn will_typescript_module() -> InternalModule {
    InternalModule::native("pete:will")
        .with_function("say", ts_say, 1)
        .with_function("listFiles", ts_list_files, 0)
        .with_function("readSourceFile", ts_read_source_file, 2)
        .with_function("readFile", ts_read_source_file, 2)
        .with_function("searchSource", ts_search_source, 2)
        .with_function("grepSource", ts_grep_source, 2)
        .with_function("readRecentTimeline", ts_read_recent_timeline, 1)
        .with_function("readRecentConversation", ts_read_recent_conversation, 1)
        .with_function("recall", ts_recall, 2)
        .with_function("inspectGraphNode", ts_inspect_graph_node, 1)
        .with_function("neighbors", ts_neighbors, 2)
        .with_function("look", ts_look, 0)
        .with_function("listenRecent", ts_listen_recent, 1)
        .with_function("recentFaces", ts_recent_faces, 1)
        .with_function("recentVoices", ts_recent_voices, 1)
        .with_function("recognizeFace", ts_recognize_face, 2)
        .with_function("recognizeVoice", ts_recognize_voice, 2)
        .with_function("setFace", ts_set_face, 1)
        .with_function("note", ts_note, 1)
        .with_function("remember", ts_remember, 1)
        .build()
}

fn command_value(interp: &mut Interpreter, value: Value) -> Result<Guarded, JsError> {
    let guard = api::create_guard(interp);
    let value = api::create_from_json(interp, &guard, &value)?;
    Ok(Guarded::with_guard(value, guard))
}

fn string_arg(args: &[JsValue], index: usize) -> String {
    args.get(index)
        .and_then(JsValue::as_str)
        .unwrap_or_default()
        .to_string()
}

fn optional_positive_integer_arg(args: &[JsValue], index: usize) -> Option<usize> {
    args.get(index)
        .and_then(JsValue::as_number)
        .filter(|number| number.is_finite() && *number > 0.0)
        .map(|number| number.floor() as usize)
}

fn non_negative_integer_arg(args: &[JsValue], index: usize) -> usize {
    args.get(index)
        .and_then(JsValue::as_number)
        .filter(|number| number.is_finite() && *number >= 0.0)
        .map(|number| number.floor() as usize)
        .unwrap_or(0)
}

fn optional_limit_payload(kind: &str, args: &[JsValue]) -> Value {
    let mut value = json!({ "kind": kind });
    if let Some(limit) = optional_positive_integer_arg(args, 0) {
        value["limit"] = json!(limit);
    }
    value
}

fn ts_say(interp: &mut Interpreter, _this: JsValue, args: &[JsValue]) -> Result<Guarded, JsError> {
    command_value(
        interp,
        json!({ "kind": "say", "text": string_arg(args, 0) }),
    )
}

fn ts_list_files(
    interp: &mut Interpreter,
    _this: JsValue,
    _args: &[JsValue],
) -> Result<Guarded, JsError> {
    command_value(interp, json!({ "kind": "list_files" }))
}

fn ts_read_source_file(
    interp: &mut Interpreter,
    _this: JsValue,
    args: &[JsValue],
) -> Result<Guarded, JsError> {
    let mut value = json!({ "kind": "read_source_file", "file": string_arg(args, 0) });
    if let Some(page) = optional_positive_integer_arg(args, 1) {
        value["page"] = json!(page);
    }
    command_value(interp, value)
}

fn ts_search_source(
    interp: &mut Interpreter,
    _this: JsValue,
    args: &[JsValue],
) -> Result<Guarded, JsError> {
    let mut value = json!({ "kind": "search_source", "query": string_arg(args, 0) });
    if let Some(limit) = optional_positive_integer_arg(args, 1) {
        value["limit"] = json!(limit);
    }
    command_value(interp, value)
}

fn ts_grep_source(
    interp: &mut Interpreter,
    _this: JsValue,
    args: &[JsValue],
) -> Result<Guarded, JsError> {
    let mut value = json!({ "kind": "grep_source", "pattern": string_arg(args, 0) });
    if let Some(limit) = optional_positive_integer_arg(args, 1) {
        value["limit"] = json!(limit);
    }
    command_value(interp, value)
}

fn ts_read_recent_timeline(
    interp: &mut Interpreter,
    _this: JsValue,
    args: &[JsValue],
) -> Result<Guarded, JsError> {
    command_value(interp, optional_limit_payload("read_recent_timeline", args))
}

fn ts_read_recent_conversation(
    interp: &mut Interpreter,
    _this: JsValue,
    args: &[JsValue],
) -> Result<Guarded, JsError> {
    command_value(
        interp,
        optional_limit_payload("read_recent_conversation", args),
    )
}

fn ts_recall(
    interp: &mut Interpreter,
    _this: JsValue,
    args: &[JsValue],
) -> Result<Guarded, JsError> {
    let mut value = json!({ "kind": "recall", "query": string_arg(args, 0) });
    if let Some(limit) = optional_positive_integer_arg(args, 1) {
        value["limit"] = json!(limit);
    }
    command_value(interp, value)
}

fn ts_inspect_graph_node(
    interp: &mut Interpreter,
    _this: JsValue,
    args: &[JsValue],
) -> Result<Guarded, JsError> {
    command_value(
        interp,
        json!({ "kind": "inspect_graph_node", "id": string_arg(args, 0) }),
    )
}

fn ts_neighbors(
    interp: &mut Interpreter,
    _this: JsValue,
    args: &[JsValue],
) -> Result<Guarded, JsError> {
    let mut value = json!({ "kind": "neighbors", "id": string_arg(args, 0) });
    if let Some(depth) = optional_positive_integer_arg(args, 1) {
        value["depth"] = json!(depth);
    }
    command_value(interp, value)
}

fn ts_look(
    interp: &mut Interpreter,
    _this: JsValue,
    _args: &[JsValue],
) -> Result<Guarded, JsError> {
    command_value(interp, json!({ "kind": "look" }))
}

fn ts_listen_recent(
    interp: &mut Interpreter,
    _this: JsValue,
    args: &[JsValue],
) -> Result<Guarded, JsError> {
    command_value(interp, optional_limit_payload("listen_recent", args))
}

fn ts_recent_faces(
    interp: &mut Interpreter,
    _this: JsValue,
    args: &[JsValue],
) -> Result<Guarded, JsError> {
    command_value(interp, optional_limit_payload("recent_faces", args))
}

fn ts_recent_voices(
    interp: &mut Interpreter,
    _this: JsValue,
    args: &[JsValue],
) -> Result<Guarded, JsError> {
    command_value(interp, optional_limit_payload("recent_voices", args))
}

fn ts_recognize_face(
    interp: &mut Interpreter,
    _this: JsValue,
    args: &[JsValue],
) -> Result<Guarded, JsError> {
    let (index, name) = if args.get(1).is_some() {
        (non_negative_integer_arg(args, 0), string_arg(args, 1))
    } else {
        (0, string_arg(args, 0))
    };
    command_value(
        interp,
        json!({ "kind": "recognize_face", "index": index, "name": name }),
    )
}

fn ts_recognize_voice(
    interp: &mut Interpreter,
    _this: JsValue,
    args: &[JsValue],
) -> Result<Guarded, JsError> {
    let (index, name) = if args.get(1).is_some() {
        (non_negative_integer_arg(args, 0), string_arg(args, 1))
    } else {
        (0, string_arg(args, 0))
    };
    command_value(
        interp,
        json!({ "kind": "recognize_voice", "index": index, "name": name }),
    )
}

fn ts_set_face(
    interp: &mut Interpreter,
    _this: JsValue,
    args: &[JsValue],
) -> Result<Guarded, JsError> {
    command_value(
        interp,
        json!({ "kind": "set_face", "emoji": string_arg(args, 0) }),
    )
}

fn ts_note(interp: &mut Interpreter, _this: JsValue, args: &[JsValue]) -> Result<Guarded, JsError> {
    command_value(
        interp,
        json!({ "kind": "note", "text": string_arg(args, 0) }),
    )
}

fn ts_remember(
    interp: &mut Interpreter,
    _this: JsValue,
    args: &[JsValue],
) -> Result<Guarded, JsError> {
    command_value(
        interp,
        json!({ "kind": "remember", "text": string_arg(args, 0) }),
    )
}

async fn store_thought_sensation(
    observer: &SensationGraphObserver,
    memory: &dyn Memory,
    combobulation: &GraphLatestCombobulation,
    thought: &str,
) {
    let occurred_at = Utc::now();
    let summary = format!("I think: {}", thought.trim());
    let stimulus_at = parse_utc(&combobulation.formed_at).unwrap_or(occurred_at);
    let stimulus = Stimulus::with_source_sensation_ids(
        combobulation.text.clone(),
        stimulus_at,
        [combobulation.id.clone()],
    );
    let impression = Impression::new(vec![stimulus], summary, None::<String>);
    store_will_memory_sensation(observer, memory, impression, occurred_at).await;
}

async fn store_speech_intention_sensation(
    observer: &SensationGraphObserver,
    combobulation: &GraphLatestCombobulation,
    words: &str,
) {
    let Some(words) = common::non_empty_model_text(words) else {
        return;
    };
    let occurred_at = Utc::now();
    let summary = format!("I ought to say: {words}");
    let stimulus_at = parse_utc(&combobulation.formed_at).unwrap_or(occurred_at);
    let stimulus = Stimulus::with_source_sensation_ids(
        combobulation.text.clone(),
        stimulus_at,
        [combobulation.id.clone()],
    );
    let impression = Impression::new(vec![stimulus], summary, None::<String>);
    let sensation = Sensation::of_at(impression, occurred_at);
    observer.observe_sensation(&sensation).await;
}

async fn store_face_expression_sensation(
    observer: &SensationGraphObserver,
    combobulation: &GraphLatestCombobulation,
    emoji: &str,
) {
    let Some(emoji) = common::non_empty_model_text(emoji) else {
        return;
    };
    let occurred_at = Utc::now();
    let summary = format!("I turn my face into a {}.", emoji.trim());
    let stimulus_at = parse_utc(&combobulation.formed_at).unwrap_or(occurred_at);
    let stimulus = Stimulus::with_source_sensation_ids(
        combobulation.text.clone(),
        stimulus_at,
        [combobulation.id.clone()],
    );
    let impression = Impression::new(vec![stimulus], summary, Some(emoji.trim().to_string()));
    let sensation = Sensation::of_at(impression, occurred_at);
    observer.observe_sensation(&sensation).await;
}

async fn store_note_sensation(
    observer: &SensationGraphObserver,
    memory: &dyn Memory,
    combobulation: &GraphLatestCombobulation,
    prefix: &str,
    text: &str,
) {
    let Some(text) = common::non_empty_model_text(text) else {
        return;
    };
    let occurred_at = Utc::now();
    let summary = format!("{prefix}: {}", text.trim());
    let stimulus_at = parse_utc(&combobulation.formed_at).unwrap_or(occurred_at);
    let stimulus = Stimulus::with_source_sensation_ids(
        combobulation.text.clone(),
        stimulus_at,
        [combobulation.id.clone()],
    );
    let impression = Impression::new(vec![stimulus], summary, None::<String>);
    store_will_memory_sensation(observer, memory, impression, occurred_at).await;
}

async fn store_will_memory_sensation(
    observer: &SensationGraphObserver,
    memory: &dyn Memory,
    impression: Impression<String>,
    occurred_at: DateTime<Utc>,
) {
    let sensation = Sensation::of_at(impression.clone(), occurred_at);
    observer.observe_sensation(&sensation).await;
    if let Err(err) = store_serializable_will_impression(memory, &impression).await {
        warn!(
            error = %format!("{err:#}"),
            summary = %impression.summary,
            "will memory store failed"
        );
    }
}

async fn store_serializable_will_impression(
    memory: &dyn Memory,
    impression: &Impression<String>,
) -> anyhow::Result<()> {
    let stimuli = impression
        .stimuli
        .iter()
        .map(|stimulus| {
            Ok(Stimulus {
                what: serde_json::to_value(&stimulus.what)?,
                timestamp: stimulus.timestamp,
                source_sensation_ids: stimulus.source_sensation_ids.clone(),
            })
        })
        .collect::<Result<Vec<_>, serde_json::Error>>()?;
    let erased = Impression {
        stimuli,
        source_sensation_ids: impression.source_sensation_ids.clone(),
        summary: impression.summary.clone(),
        emoji: impression.emoji.clone(),
        timestamp: impression.timestamp,
    };
    memory.store(&erased).await
}

async fn store_function_result_sensation(
    observer: &SensationGraphObserver,
    combobulation: &GraphLatestCombobulation,
    func_name: &str,
    result: &str,
) {
    let occurred_at = Utc::now();
    let summary = format!("Result of {}: {}", func_name, result);
    let stimulus_at = parse_utc(&combobulation.formed_at).unwrap_or(occurred_at);
    let stimulus = Stimulus::with_source_sensation_ids(
        combobulation.text.clone(),
        stimulus_at,
        [combobulation.id.clone()],
    );
    let impression = Impression::new(vec![stimulus], summary, None::<String>);
    let sensation = Sensation::of_at(impression, occurred_at);
    observer.observe_sensation(&sensation).await;
}

async fn store_will_context_sensation(
    observer: &SensationGraphObserver,
    system_prompt: String,
    report: Option<WitReport>,
    typescript: String,
    results: Vec<WillTypeScriptResult>,
) {
    let typescript =
        (!typescript.trim().is_empty() || !results.is_empty()).then(|| WillTypeScriptExecution {
            source: typescript.trim().to_string(),
            timestamp: Utc::now().to_rfc3339(),
            results,
        });
    let context = WillContext {
        system_prompt,
        history: Vec::<ConversationEntry>::new(),
        report,
        typescript,
    };
    let sensation = Sensation::of_at(context, Utc::now());
    observer.observe_sensation(&sensation).await;
}

fn parse_utc(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use psyche::GraphStore;
    use serde_json::Value;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct MockGraph {
        records: Mutex<Vec<Value>>,
    }

    #[async_trait]
    impl GraphStore for MockGraph {
        async fn store_data(&self, data: &Value) -> anyhow::Result<()> {
            self.records.lock().unwrap().push(data.clone());
            Ok(())
        }
    }

    #[derive(Default)]
    struct MockMemory {
        impressions: Mutex<Vec<Impression<Value>>>,
    }

    #[async_trait]
    impl Memory for MockMemory {
        async fn store(&self, impression: &Impression<Value>) -> anyhow::Result<()> {
            self.impressions.lock().unwrap().push(impression.clone());
            Ok(())
        }
    }

    fn latest() -> GraphLatestCombobulation {
        GraphLatestCombobulation {
            id: "awareness:1".into(),
            text: "I notice someone nearby.".into(),
            emoji: Some("🙂".into()),
            formed_at: "2026-05-07T12:00:00Z".into(),
        }
    }

    fn recent_conversation() -> Vec<GraphSensationTimelineItem> {
        vec![GraphSensationTimelineItem {
            id: "sensation:heard:1".into(),
            labels: vec!["GraphNode".into(), "Sensation".into()],
            kind: "text".into(),
            text: "I heard: please inspect the Will.".into(),
            occurred_at: "2026-05-07T12:01:00Z".into(),
            formed_at: Some("2026-05-07T12:01:01Z".into()),
        }]
    }

    #[tokio::test]
    async fn will_thoughts_notes_and_memories_are_sent_to_graph_and_memory() {
        let graph = Arc::new(MockGraph::default());
        let observer = SensationGraphObserver::new(graph.clone());
        let memory = MockMemory::default();
        let latest = latest();

        store_thought_sensation(&observer, &memory, &latest, "inspect the room").await;
        store_note_sensation(&observer, &memory, &latest, "I note", "the lights changed").await;
        store_note_sensation(
            &observer,
            &memory,
            &latest,
            "I remember",
            "the door was open",
        )
        .await;

        let graph_records = graph.records.lock().unwrap();
        assert_eq!(graph_records.len(), 3);
        assert!(
            graph_records
                .iter()
                .all(|record| { record.get("op").and_then(Value::as_str) == Some("merge_graph") })
        );

        let impressions = memory.impressions.lock().unwrap();
        let summaries = impressions
            .iter()
            .map(|impression| impression.summary.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            summaries,
            vec![
                "I think: inspect the room",
                "I note: the lights changed",
                "I remember: the door was open"
            ]
        );
        let expected_sources = vec!["awareness:1".to_string()];
        assert!(impressions.iter().all(|impression| {
            impression.source_sensation_ids == expected_sources
                && impression.stimuli[0].source_sensation_ids == expected_sources
        }));
    }

    #[test]
    fn will_prompt_uses_instruction_json_with_current_conversation() {
        let prompt = will_instruction_prompt(&latest(), None, None, &recent_conversation());

        assert!(prompt.contains("You are PETE"));
        assert!(prompt.contains("Pete Daringsby, an artificial intelligence, not simply an LLM"));
        assert!(!prompt.contains("You are the Will"));
        assert!(prompt.contains("This is the situation as you understand it:"));
        assert!(prompt.contains("Current conversation:"));
        assert!(prompt.contains("I heard: please inspect the Will."));
        assert!(prompt.contains("current conversation is context"));
        assert!(prompt.contains("You do not need to maintain the conversation"));
        assert!(prompt.contains("Keep say(text) for emergencies"));
        assert!(prompt.contains("Return only a JSON object"));
        assert!(prompt.contains("\"thought\""));
        assert!(prompt.contains("\"typescript\""));
        assert!(prompt.contains("thought process, intended actions, desires"));
        assert!(prompt.contains("import command builders from \"pete:will\""));
        assert!(prompt.contains("listFiles()"));
        assert!(prompt.contains("readSourceFile(path: string, page?: number)"));
        assert!(prompt.contains("searchSource(query: string, limit?: number)"));
        assert!(prompt.contains("recall(query: string, limit?: number)"));
        assert!(prompt.contains("recentFaces(limit?: number)"));
        assert!(prompt.contains("recognizeFace(index: number, name: string)"));
        assert!(prompt.contains("recognizeVoice(index: number, name: string)"));
        assert!(prompt.contains("setFace(emoji: string)"));
        assert!(prompt.contains("say(text: string) - emergency speech"));
        assert!(!prompt.contains("<thought>"));
        assert!(!prompt.contains("<function"));
    }

    #[test]
    fn will_prompt_includes_empty_current_conversation_section() {
        let prompt = will_instruction_prompt(&latest(), None, None, &[]);

        assert!(prompt.contains("Current conversation:"));
        assert!(prompt.contains("(no current conversation)"));
    }

    #[test]
    fn parses_structured_action_with_typescript() {
        let action = parse_will_action(
            r#"{"thought":"Inspect my source next.","typescript":"import { listFiles, say } from \"pete:will\";\nconst target: string = 'source';\n[listFiles(), say(`I am checking my ${target}.`)]"}"#,
        )
        .unwrap();

        assert_eq!(action.thought, "Inspect my source next.");
        assert_eq!(
            action.commands,
            vec![
                TypeScriptCommand::ListFiles,
                TypeScriptCommand::Say("I am checking my source.".into())
            ]
        );
    }

    #[test]
    fn parses_json_inside_markdown_fence() {
        let action = parse_will_action(
            "```json\n{\"thought\":\"Read a file.\",\"typescript\":\"import { readSourceFile } from \\\"pete:will\\\";\\nreadSourceFile('pete/src/bin/will.rs', 1 + 1)\"}\n```",
        )
        .unwrap();

        assert_eq!(action.thought, "Read a file.");
        assert_eq!(
            action.commands,
            vec![TypeScriptCommand::ReadSourceFile {
                file: "pete/src/bin/will.rs".into(),
                page: 2
            }]
        );
    }

    #[test]
    fn typescript_errors_are_rejected() {
        let err = parse_will_action(
            r#"{"thought":"Try an unsupported call.","typescript":"fetch('http://example.test')"}"#,
        )
        .unwrap_err();

        assert!(err.to_string().contains("fetch"));
    }

    #[test]
    fn read_file_alias_defaults_to_first_page() {
        let action = parse_will_action(
            r#"{"thought":"Use the alias.","typescript":"import { readFile } from \"pete:will\";\nreadFile(\"psyche/src/lib.rs\")"}"#,
        )
        .unwrap();

        assert_eq!(
            action.commands,
            vec![TypeScriptCommand::ReadSourceFile {
                file: "psyche/src/lib.rs".into(),
                page: 1
            }]
        );
    }

    #[test]
    fn parses_extended_typescript_functions() {
        let action = parse_will_action(
            r#"{"thought":"I want to inspect memory and my environment.","typescript":"import { searchSource, grepSource, readRecentTimeline, readRecentConversation, listenRecent, recentFaces, recentVoices, recall, inspectGraphNode, neighbors, look, recognizeFace, recognizeVoice, setFace, note, remember } from \"pete:will\";\n[\n  searchSource('WillProcessor', 3),\n  grepSource('latest_function_results'),\n  readRecentTimeline(4),\n  readRecentConversation(5),\n  listenRecent(2),\n  recentFaces(3),\n  recentVoices(4),\n  recall('faces and voices', 6),\n  inspectGraphNode('node:1'),\n  neighbors('node:1', 2),\n  look(),\n  recognizeFace(0, 'Travis'),\n  recognizeVoice('Travis'),\n  setFace('🤔'),\n  note('check source navigation'),\n  remember('source search exists now')\n]"}"#,
        )
        .unwrap();

        assert_eq!(
            action.commands,
            vec![
                TypeScriptCommand::SearchSource {
                    query: "WillProcessor".into(),
                    limit: 3
                },
                TypeScriptCommand::GrepSource {
                    pattern: "latest_function_results".into(),
                    limit: 12
                },
                TypeScriptCommand::ReadRecentTimeline { limit: 4 },
                TypeScriptCommand::ReadRecentConversation { limit: 5 },
                TypeScriptCommand::ListenRecent { limit: 2 },
                TypeScriptCommand::RecentFaces { limit: 3 },
                TypeScriptCommand::RecentVoices { limit: 4 },
                TypeScriptCommand::Recall {
                    query: "faces and voices".into(),
                    limit: 6
                },
                TypeScriptCommand::InspectGraphNode {
                    id: "node:1".into()
                },
                TypeScriptCommand::Neighbors {
                    id: "node:1".into(),
                    depth: 2
                },
                TypeScriptCommand::Look,
                TypeScriptCommand::RecognizeFace {
                    index: 0,
                    name: "Travis".into()
                },
                TypeScriptCommand::RecognizeVoice {
                    index: 0,
                    name: "Travis".into()
                },
                TypeScriptCommand::SetFace("🤔".into()),
                TypeScriptCommand::Note("check source navigation".into()),
                TypeScriptCommand::Remember("source search exists now".into()),
            ]
        );
    }

    #[test]
    fn parses_recent_will_todo_search_payloads() {
        let first = parse_will_action(
            r#"Will: {"thought":"I will scan the repository for TODO markers to spot unfinished work, and also peek at recent first-person sensations to stay aware of my own context. This helps me stay busy and keeps the system from idling.","typescript":"import { listFiles, grepSource, readRecentTimeline } from \"pete:will\";\n[listFiles(), grepSource('TODO', 10), readRecentTimeline(5)]"}"#,
        )
        .unwrap();

        assert_eq!(
            first.commands,
            vec![
                TypeScriptCommand::ListFiles,
                TypeScriptCommand::GrepSource {
                    pattern: "TODO".into(),
                    limit: 10
                },
                TypeScriptCommand::ReadRecentTimeline { limit: 5 },
            ]
        );

        let second = parse_will_action(
            r#"Will: {"thought":"I want to find TODO markers in the source code to identify unfinished work. No speech needed.","typescript":"import { grepSource } from \"pete:will\";\ngrepSource(\"TODO\", 10)"}"#,
        )
        .unwrap();

        assert_eq!(
            second.commands,
            vec![TypeScriptCommand::GrepSource {
                pattern: "TODO".into(),
                limit: 10
            }]
        );
    }

    #[test]
    fn unstructured_response_becomes_thought_without_commands() {
        let action = parse_will_action("Think about source navigation.").unwrap();

        assert_eq!(action.thought, "Think about source navigation.");
        assert!(action.typescript.is_empty());
        assert!(action.commands.is_empty());
    }

    #[test]
    fn quoted_empty_response_does_not_become_thought() {
        let action = parse_will_action(r#""""#).unwrap();

        assert!(action.thought.is_empty());
        assert!(action.typescript.is_empty());
        assert!(action.commands.is_empty());
    }

    #[test]
    fn quoted_empty_speech_is_ignored() {
        let action = parse_will_action(
            r#"{"thought":"Wait.","typescript":"import { say } from \"pete:will\";\n[say(''), say('\"\"'), say('still here')]"}"#,
        )
        .unwrap();

        assert_eq!(action.thought, "Wait.");
        assert_eq!(
            action.commands,
            vec![TypeScriptCommand::Say("still here".into())]
        );
    }
}

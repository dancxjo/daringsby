use std::sync::OnceLock;
use std::time::Duration;

use anyhow::Context;
use chrono::{DateTime, Utc};
use clap::Parser;
use dotenvy::dotenv;
use lingproc::{Doer, LlmInstruction, Vectorizer};
use pete::{EventBus, init_logging, ollama_provider_from_args};
use psyche::{
    ConversationEntry, GraphLatestCombobulation, GraphNodeDetails, GraphSensationTimelineItem,
    GraphSnapshot, Impression, Neo4jClient, QdrantClient, Sensation, SensationGraphObserver,
    SensationObserver, Stimulus, WillContext, WitReport, with_default_system_prompt,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio::time::{MissedTickBehavior, interval};
use tracing::{error, info, trace};

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
    /// URL of the wits Ollama server.
    #[arg(long, env = "WITS_HOST", default_value = "http://localhost:11434")]
    wits_host: String,
    /// Model name to use for the Will.
    #[arg(long, env = "WITS_MODEL", default_value = "gpt-oss")]
    wits_model: String,
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
    let qdrant = std::sync::Arc::new(QdrantClient::new(cli.qdrant_url.clone()));
    let observer = SensationGraphObserver::new(graph.clone());
    let doer = ollama_provider_from_args(&cli.wits_host, &cli.wits_model)?;
    let processor = WillProcessor {
        doer,
        graph: graph.clone(),
        qdrant,
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
        store_thought_sensation(observer, &combobulation, &action.thought).await;
    }

    for command in action.commands.iter() {
        match command {
            JavascriptCommand::Say(text) => {
                store_speech_intention_sensation(observer, &combobulation, text).await;
            }
            JavascriptCommand::SetFace(emoji) => {
                store_face_expression_sensation(observer, &combobulation, emoji).await;
            }
            JavascriptCommand::Note(text) => {
                store_note_sensation(observer, &combobulation, "I note", text).await;
            }
            JavascriptCommand::Remember(text) => {
                store_note_sensation(observer, &combobulation, "I remember", text).await;
            }
            _ => {
                let (name, summary) = processor.execute_command(command).await;
                store_function_result_sensation(observer, &combobulation, name, &summary).await;
            }
        }
    }

    info!(
        combobulation_id = %combobulation.id,
        thought = %action.thought,
        javascript = %action.javascript,
        "will chose action"
    );
    store_will_context_sensation(observer, action.system_prompt, action.report).await;
    Ok(Some(combobulation.id))
}

struct WillProcessor {
    doer: lingproc::OllamaProvider,
    graph: std::sync::Arc<Neo4jClient>,
    qdrant: std::sync::Arc<QdrantClient>,
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

    async fn execute_command(&self, command: &JavascriptCommand) -> (&'static str, String) {
        match command {
            JavascriptCommand::ListFiles => ("list_files", execute_list_files()),
            JavascriptCommand::ReadSourceFile { file, page } => {
                ("read_source_file", execute_read_source_file(file, *page))
            }
            JavascriptCommand::SearchSource { query, limit } => {
                ("search_source", execute_search_source(query, *limit))
            }
            JavascriptCommand::GrepSource { pattern, limit } => {
                ("grep_source", execute_grep_source(pattern, *limit))
            }
            JavascriptCommand::ReadRecentTimeline { limit } => (
                "read_recent_timeline",
                self.read_recent_timeline(*limit)
                    .await
                    .unwrap_or_else(error_text),
            ),
            JavascriptCommand::ReadRecentConversation { limit } => (
                "read_recent_conversation",
                self.read_recent_conversation(*limit)
                    .await
                    .unwrap_or_else(error_text),
            ),
            JavascriptCommand::Recall { query, limit } => (
                "recall",
                self.recall(query, *limit).await.unwrap_or_else(error_text),
            ),
            JavascriptCommand::InspectGraphNode { id } => (
                "inspect_graph_node",
                self.inspect_graph_node(id).await.unwrap_or_else(error_text),
            ),
            JavascriptCommand::Neighbors { id, depth } => (
                "neighbors",
                self.neighbors(id, *depth).await.unwrap_or_else(error_text),
            ),
            JavascriptCommand::Look => ("look", self.look().await.unwrap_or_else(error_text)),
            JavascriptCommand::ListenRecent { limit } => (
                "listen_recent",
                self.listen_recent(*limit).await.unwrap_or_else(error_text),
            ),
            JavascriptCommand::Say(_)
            | JavascriptCommand::SetFace(_)
            | JavascriptCommand::Note(_)
            | JavascriptCommand::Remember(_) => ("noop", "No function result.".into()),
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
        let vector = self.doer.vectorize(query).await?;
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
         Recent conversation:\n\
         {}\n\n\
         You are the Will. Decide what the system should work on next. \
         You are not the conversation manager; do not answer the user conversationally unless you intentionally queue speech with say(text). \
         Return only a JSON object with exactly these fields in this order:\n\
         {{\"thought\":\"a concise explanation of your thought process, intended actions, desires, and why the javascript is or is not needed\",\"javascript\":\"a short program using only the allowed functions\"}}\n\n\
         The javascript field may call only these functions:\n\
         say(text) - inserts speech into the queue.\n\
         list_files() - lists the extant source files.\n\
         read_source_file(path, page) - reads one source file page; page is optional and defaults to 1.\n\n\
         search_source(query, limit) - searches source files for a plain-language or literal query.\n\
         grep_source(pattern, limit) - searches source files for a literal text pattern.\n\
         read_recent_timeline(limit) - reads recent first-person sensations.\n\
         read_recent_conversation(limit) - reads recent hearing and speaking events.\n\
         listen_recent(limit) - reads recent hearing-only events.\n\
         recall(query, limit) - searches long-term memory for related impressions.\n\
         inspect_graph_node(id) - reads one graph node and nearby relationships.\n\
         neighbors(id, depth) - reads graph neighbors up to depth 2.\n\
         look() - reads the latest image description.\n\
         set_face(emoji) - turns your face into an emoji.\n\
         note(text) - records a private self-note.\n\
         remember(text) - records something you want future Will cycles to remember.\n\n\
         Use an empty string for javascript when no action is needed, but do try to keep yourself busy and prevent yourself from being idle. \
         You may call other functions, define functions, or import modules, but do not use markdown, or include text outside the JSON object. \
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
        return "(no recent conversation)".into();
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
    javascript: String,
    commands: Vec<JavascriptCommand>,
    system_prompt: String,
    report: Option<WitReport>,
}

#[derive(Deserialize)]
struct WillActionPayload {
    #[serde(default)]
    thought: String,
    #[serde(default)]
    javascript: String,
}

#[derive(Debug, PartialEq, Eq)]
enum JavascriptCommand {
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
    SetFace(String),
    Note(String),
    Remember(String),
}

fn parse_will_action(raw: &str) -> anyhow::Result<WillAction> {
    let payload = parse_will_action_payload(raw)?;
    let commands = execute_javascript_commands(&payload.javascript)?;

    Ok(WillAction {
        thought: common::non_empty_model_text(&payload.thought)
            .unwrap_or_default()
            .to_string(),
        javascript: payload.javascript.trim().to_string(),
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
        javascript: String::new(),
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
enum JavascriptCommandPayload {
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

fn execute_javascript_commands(script: &str) -> anyhow::Result<Vec<JavascriptCommand>> {
    if script.trim().is_empty() {
        return Ok(Vec::new());
    }

    let wrapped = format!(
        r#"
const __daringsbyCommands = [];
function __daringsbyString(value) {{
  return value === undefined || value === null ? "" : String(value);
}}
function __daringsbyPositiveInteger(value) {{
  const number = Number(value);
  return Number.isFinite(number) && number > 0 ? Math.floor(number) : 1;
}}
function say(text) {{
  __daringsbyCommands.push({{kind: "say", text: __daringsbyString(text)}});
}}
function list_files() {{
  __daringsbyCommands.push({{kind: "list_files"}});
}}
function read_source_file(path, page) {{
  const command = {{kind: "read_source_file", file: __daringsbyString(path)}};
  if (page !== undefined) command.page = __daringsbyPositiveInteger(page);
  __daringsbyCommands.push(command);
}}
const read_file = read_source_file;
function search_source(query, limit) {{
  const command = {{kind: "search_source", query: __daringsbyString(query)}};
  if (limit !== undefined) command.limit = __daringsbyPositiveInteger(limit);
  __daringsbyCommands.push(command);
}}
function grep_source(pattern, limit) {{
  const command = {{kind: "grep_source", pattern: __daringsbyString(pattern)}};
  if (limit !== undefined) command.limit = __daringsbyPositiveInteger(limit);
  __daringsbyCommands.push(command);
}}
function read_recent_timeline(limit) {{
  const command = {{kind: "read_recent_timeline"}};
  if (limit !== undefined) command.limit = __daringsbyPositiveInteger(limit);
  __daringsbyCommands.push(command);
}}
function read_recent_conversation(limit) {{
  const command = {{kind: "read_recent_conversation"}};
  if (limit !== undefined) command.limit = __daringsbyPositiveInteger(limit);
  __daringsbyCommands.push(command);
}}
function recall(query, limit) {{
  const command = {{kind: "recall", query: __daringsbyString(query)}};
  if (limit !== undefined) command.limit = __daringsbyPositiveInteger(limit);
  __daringsbyCommands.push(command);
}}
function inspect_graph_node(id) {{
  __daringsbyCommands.push({{kind: "inspect_graph_node", id: __daringsbyString(id)}});
}}
function neighbors(id, depth) {{
  const command = {{kind: "neighbors", id: __daringsbyString(id)}};
  if (depth !== undefined) command.depth = __daringsbyPositiveInteger(depth);
  __daringsbyCommands.push(command);
}}
function look() {{
  __daringsbyCommands.push({{kind: "look"}});
}}
function listen_recent(limit) {{
  const command = {{kind: "listen_recent"}};
  if (limit !== undefined) command.limit = __daringsbyPositiveInteger(limit);
  __daringsbyCommands.push(command);
}}
function set_face(emoji) {{
  __daringsbyCommands.push({{kind: "set_face", emoji: __daringsbyString(emoji)}});
}}
function note(text) {{
  __daringsbyCommands.push({{kind: "note", text: __daringsbyString(text)}});
}}
function remember(text) {{
  __daringsbyCommands.push({{kind: "remember", text: __daringsbyString(text)}});
}}
{script}
JSON.stringify(__daringsbyCommands);
"#
    );

    let value = javascript::evaluate_script(wrapped, None::<&std::path::Path>)
        .map_err(|err| anyhow::anyhow!("javascript evaluation failed: {err}"))?;
    let json = match value {
        javascript::Value::String(units) => String::from_utf16_lossy(&units),
        other => anyhow::bail!("javascript command program returned non-string value: {other}"),
    };
    let payloads: Vec<JavascriptCommandPayload> = serde_json::from_str(&json)?;
    Ok(payloads
        .into_iter()
        .filter_map(|payload| match payload {
            JavascriptCommandPayload::Say { text } => common::non_empty_model_text(&text)
                .map(|text| JavascriptCommand::Say(text.to_string())),
            JavascriptCommandPayload::ListFiles => Some(JavascriptCommand::ListFiles),
            JavascriptCommandPayload::ReadSourceFile { file, page } => {
                let file = file.trim();
                (!file.is_empty()).then(|| JavascriptCommand::ReadSourceFile {
                    file: file.to_string(),
                    page: page.unwrap_or(1).max(1),
                })
            }
            JavascriptCommandPayload::SearchSource { query, limit } => {
                common::non_empty_model_text(&query).map(|query| JavascriptCommand::SearchSource {
                    query: query.to_string(),
                    limit: limit.unwrap_or(12).max(1),
                })
            }
            JavascriptCommandPayload::GrepSource { pattern, limit } => {
                common::non_empty_model_text(&pattern).map(|pattern| {
                    JavascriptCommand::GrepSource {
                        pattern: pattern.to_string(),
                        limit: limit.unwrap_or(12).max(1),
                    }
                })
            }
            JavascriptCommandPayload::ReadRecentTimeline { limit } => {
                Some(JavascriptCommand::ReadRecentTimeline {
                    limit: limit.unwrap_or(12).max(1),
                })
            }
            JavascriptCommandPayload::ReadRecentConversation { limit } => {
                Some(JavascriptCommand::ReadRecentConversation {
                    limit: limit.unwrap_or(12).max(1),
                })
            }
            JavascriptCommandPayload::Recall { query, limit } => {
                common::non_empty_model_text(&query).map(|query| JavascriptCommand::Recall {
                    query: query.to_string(),
                    limit: limit.unwrap_or(6).max(1),
                })
            }
            JavascriptCommandPayload::InspectGraphNode { id } => common::non_empty_model_text(&id)
                .map(|id| JavascriptCommand::InspectGraphNode { id: id.to_string() }),
            JavascriptCommandPayload::Neighbors { id, depth } => common::non_empty_model_text(&id)
                .map(|id| JavascriptCommand::Neighbors {
                    id: id.to_string(),
                    depth: depth.unwrap_or(1).clamp(1, 2),
                }),
            JavascriptCommandPayload::Look => Some(JavascriptCommand::Look),
            JavascriptCommandPayload::ListenRecent { limit } => {
                Some(JavascriptCommand::ListenRecent {
                    limit: limit.unwrap_or(10).max(1),
                })
            }
            JavascriptCommandPayload::SetFace { emoji } => common::non_empty_model_text(&emoji)
                .map(|emoji| JavascriptCommand::SetFace(emoji.to_string())),
            JavascriptCommandPayload::Note { text } => common::non_empty_model_text(&text)
                .map(|text| JavascriptCommand::Note(text.to_string())),
            JavascriptCommandPayload::Remember { text } => common::non_empty_model_text(&text)
                .map(|text| JavascriptCommand::Remember(text.to_string())),
        })
        .collect())
}

async fn store_thought_sensation(
    observer: &SensationGraphObserver,
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
    let sensation = Sensation::of_at(impression, occurred_at);
    observer.observe_sensation(&sensation).await;
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
    let sensation = Sensation::of_at(impression, occurred_at);
    observer.observe_sensation(&sensation).await;
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
) {
    let context = WillContext {
        system_prompt,
        history: Vec::<ConversationEntry>::new(),
        report,
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

    #[test]
    fn will_prompt_uses_instruction_json_with_recent_conversation() {
        let prompt = will_instruction_prompt(&latest(), None, None, &recent_conversation());

        assert!(prompt.contains("You are PETE"));
        assert!(prompt.contains("This is the situation as you understand it:"));
        assert!(prompt.contains("Recent conversation:"));
        assert!(prompt.contains("I heard: please inspect the Will."));
        assert!(prompt.contains("Return only a JSON object"));
        assert!(prompt.contains("\"thought\""));
        assert!(prompt.contains("\"javascript\""));
        assert!(prompt.contains("thought process, intended actions, desires"));
        assert!(prompt.contains("list_files()"));
        assert!(prompt.contains("read_source_file(path, page)"));
        assert!(prompt.contains("search_source(query, limit)"));
        assert!(prompt.contains("recall(query, limit)"));
        assert!(prompt.contains("set_face(emoji)"));
        assert!(!prompt.contains("<thought>"));
        assert!(!prompt.contains("<function"));
    }

    #[test]
    fn will_prompt_includes_empty_recent_conversation_section() {
        let prompt = will_instruction_prompt(&latest(), None, None, &[]);

        assert!(prompt.contains("Recent conversation:"));
        assert!(prompt.contains("(no recent conversation)"));
    }

    #[test]
    fn parses_structured_action_with_javascript() {
        let action = parse_will_action(
            r#"{"thought":"Inspect my source next.","javascript":"const target = 'source'; list_files(); say(`I am checking my ${target}.`);"}"#,
        )
        .unwrap();

        assert_eq!(action.thought, "Inspect my source next.");
        assert_eq!(
            action.commands,
            vec![
                JavascriptCommand::ListFiles,
                JavascriptCommand::Say("I am checking my source.".into())
            ]
        );
    }

    #[test]
    fn parses_json_inside_markdown_fence() {
        let action = parse_will_action(
            "```json\n{\"thought\":\"Read a file.\",\"javascript\":\"read_source_file('pete/src/bin/will.rs', 1 + 1);\"}\n```",
        )
        .unwrap();

        assert_eq!(action.thought, "Read a file.");
        assert_eq!(
            action.commands,
            vec![JavascriptCommand::ReadSourceFile {
                file: "pete/src/bin/will.rs".into(),
                page: 2
            }]
        );
    }

    #[test]
    fn javascript_errors_are_rejected() {
        let err = parse_will_action(
            r#"{"thought":"Try an unsupported call.","javascript":"fetch('http://example.test'); say('hi');"}"#,
        )
        .unwrap_err();

        assert!(err.to_string().contains("fetch"));
    }

    #[test]
    fn read_file_alias_defaults_to_first_page() {
        let action = parse_will_action(
            r#"{"thought":"Use the alias.","javascript":"read_file(\"psyche/src/lib.rs\");"}"#,
        )
        .unwrap();

        assert_eq!(
            action.commands,
            vec![JavascriptCommand::ReadSourceFile {
                file: "psyche/src/lib.rs".into(),
                page: 1
            }]
        );
    }

    #[test]
    fn parses_extended_javascript_functions() {
        let action = parse_will_action(
            r#"{"thought":"I want to inspect memory and my environment.","javascript":"search_source('WillProcessor', 3); grep_source('latest_function_results'); read_recent_timeline(4); read_recent_conversation(5); listen_recent(2); recall('faces and voices', 6); inspect_graph_node('node:1'); neighbors('node:1', 2); look(); set_face('🤔'); note('check source navigation'); remember('source search exists now');"}"#,
        )
        .unwrap();

        assert_eq!(
            action.commands,
            vec![
                JavascriptCommand::SearchSource {
                    query: "WillProcessor".into(),
                    limit: 3
                },
                JavascriptCommand::GrepSource {
                    pattern: "latest_function_results".into(),
                    limit: 12
                },
                JavascriptCommand::ReadRecentTimeline { limit: 4 },
                JavascriptCommand::ReadRecentConversation { limit: 5 },
                JavascriptCommand::ListenRecent { limit: 2 },
                JavascriptCommand::Recall {
                    query: "faces and voices".into(),
                    limit: 6
                },
                JavascriptCommand::InspectGraphNode {
                    id: "node:1".into()
                },
                JavascriptCommand::Neighbors {
                    id: "node:1".into(),
                    depth: 2
                },
                JavascriptCommand::Look,
                JavascriptCommand::SetFace("🤔".into()),
                JavascriptCommand::Note("check source navigation".into()),
                JavascriptCommand::Remember("source search exists now".into()),
            ]
        );
    }

    #[test]
    fn unstructured_response_becomes_thought_without_commands() {
        let action = parse_will_action("Think about source navigation.").unwrap();

        assert_eq!(action.thought, "Think about source navigation.");
        assert!(action.javascript.is_empty());
        assert!(action.commands.is_empty());
    }

    #[test]
    fn quoted_empty_response_does_not_become_thought() {
        let action = parse_will_action(r#""""#).unwrap();

        assert!(action.thought.is_empty());
        assert!(action.javascript.is_empty());
        assert!(action.commands.is_empty());
    }

    #[test]
    fn quoted_empty_speech_is_ignored() {
        let action = parse_will_action(
            r#"{"thought":"Wait.","javascript":"say(''); say('\"\"'); say('still here');"}"#,
        )
        .unwrap();

        assert_eq!(action.thought, "Wait.");
        assert_eq!(
            action.commands,
            vec![JavascriptCommand::Say("still here".into())]
        );
    }
}

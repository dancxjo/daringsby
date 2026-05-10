use std::time::Duration;

use anyhow::Context;
use chrono::{DateTime, Utc};
use clap::Parser;
use dotenvy::dotenv;
use lingproc::{Chatter, Message};
use pete::{EventBus, init_logging, ollama_provider_from_args};
use psyche::{
    GraphLatestCombobulation, GraphSensationTimelineItem, Impression, Neo4jClient, Sensation,
    SensationGraphObserver, SensationObserver, Stimulus, WillContext, ConversationEntry,
    WitReport, with_default_system_prompt,
};
use serde::Deserialize;
use std::sync::OnceLock;
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

fn execute_read_source(file: &str, page: usize) -> String {
    let map = get_source_bundle();
    if let Some(content) = map.get(file) {
        let lines: Vec<&str> = content.lines().collect();
        let chunk_size = 50;
        let start = (page.saturating_sub(1)) * chunk_size;
        if start >= lines.len() {
            return format!("File {} has only {} lines (page {} is past EOF).", file, lines.len(), page);
        }
        let end = (start + chunk_size).min(lines.len());
        let chunk = lines[start..end].join("\n");
        format!("--- {} (lines {} to {} of {}) ---\n{}\n---", file, start + 1, end, lines.len(), chunk)
    } else {
        format!("File not found: {}", file)
    }
}

fn execute_list_source() -> String {
    let map = get_source_bundle();
    let mut files: Vec<&String> = map.keys().collect();
    files.sort();
    let mut response = String::from("Available source files:\n");
    for f in files {
        response.push_str(f);
        response.push('\n');
    }
    response
}

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Choose Pete's active face expression from the latest combobulation"
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
    let observer = SensationGraphObserver::new(graph.clone());
    let doer = ollama_provider_from_args(&cli.wits_host, &cli.wits_model)?;
    let processor = WillProcessor { chatter: doer, graph: graph.clone() };

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
    
    for func in action.functions.iter() {
        let summary = if func.name == "list_source" {
            execute_list_source()
        } else if func.name == "read_source" {
            if let Some(f) = &func.file {
                let p = func.page.unwrap_or(1);
                execute_read_source(f, p)
            } else {
                "read_source failed: missing 'file' attribute".into()
            }
        } else {
            format!("Unknown function: {}", func.name)
        };
        store_function_result_sensation(observer, &combobulation, &func.name, &summary).await;
    }

    if let Some(thought_text) = action.thought.as_deref() {
        store_thought_sensation(observer, &combobulation, thought_text).await;
    }
    store_active_face_sensation(observer, &combobulation, &action.emoji).await;
    if let Some(words) = action.say.as_deref() {
        store_speech_intention_sensation(observer, &combobulation, words).await;
    }
    info!(
        combobulation_id = %combobulation.id,
        emoji = %action.emoji,
        say = action.say.as_deref().unwrap_or(""),
        thought = action.thought.as_deref().unwrap_or(""),
        "will chose action"
    );
    store_will_context_sensation(
        observer,
        action.system_prompt,
        action.history,
        action.report,
    )
    .await;
    Ok(Some(combobulation.id))
}

struct WillProcessor {
    chatter: lingproc::OllamaProvider,
    graph: std::sync::Arc<Neo4jClient>,
}

impl WillProcessor {
    async fn choose_action(
        &self,
        combobulation: &GraphLatestCombobulation,
    ) -> anyhow::Result<WillAction> {
        let vision = self.graph.latest_image_description().await.unwrap_or(None);
        let tool_results = self.graph.latest_function_results(3).await.unwrap_or_default();
        let tool_context = if tool_results.is_empty() {
            None
        } else {
            Some(tool_results.join("\n"))
        };
        
        let system_prompt = will_system_prompt(combobulation, vision, tool_context);

        let history = self
            .graph
            .conversation_timeline(None, Utc::now(), 20)
            .await
            .unwrap_or_default();
        let messages = map_conversation_to_messages(history.clone());

        let mut stream = self.chatter.chat(&system_prompt, &messages).await?;
        let mut raw = String::new();
        use tokio_stream::StreamExt;
        while let Some(chunk) = stream.next().await {
            raw.push_str(&chunk?);
        }

        let mut action = parse_will_action(raw.trim())?;
        action.system_prompt = system_prompt.clone();
        action.history = map_conversation_to_entries(history);
        action.report = Some(WitReport {
            name: "Will".into(),
            prompt: system_prompt,
            output: raw,
        });

        Ok(action)
    }
}

fn strip_utterance_content(text: &str) -> Option<(String, bool)> {
    // Returns (content, is_assistant)
    if let Some(rest) = text.strip_prefix("I heard: ") {
        return Some((rest.to_string(), false));
    }
    if text == "I hear silence." {
        return Some(("I hear silence.".to_string(), false));
    }
    if let Some(rest) = text.strip_prefix("I hear the user saying \"") {
        if let Some(end) = rest.rfind("\".") {
            return Some((rest[..end].to_string(), false));
        }
    }
    if let Some(rest) = text.strip_prefix("I hear myself saying \"") {
        if let Some(end) = rest.rfind("\".") {
            return Some((rest[..end].to_string(), true));
        }
    }
    if let Some(rest) = text.strip_prefix("I finish saying \"") {
        if let Some(end) = rest.rfind("\".") {
            return Some((rest[..end].to_string(), true));
        }
    }
    if let Some(rest) = text.strip_prefix("I finish saying: ") {
        return Some((rest.to_string(), true));
    }
    if let Some(rest) = text.strip_prefix("I say: ") {
        return Some((rest.to_string(), true));
    }
    None
}

fn map_conversation_to_entries(items: Vec<GraphSensationTimelineItem>) -> Vec<ConversationEntry> {
    items
        .into_iter()
        .filter_map(|item| {
            let (content, is_assistant) = strip_utterance_content(&item.text)?;
            Some(ConversationEntry {
                role: if is_assistant { "assistant" } else { "user" }.into(),
                content,
                timestamp: item.occurred_at,
            })
        })
        .collect()
}

fn map_conversation_to_messages(items: Vec<GraphSensationTimelineItem>) -> Vec<Message> {
    items
        .into_iter()
        .filter_map(|item| {
            let (content, is_assistant) = strip_utterance_content(&item.text)?;
            if is_assistant {
                Some(Message::assistant(&content))
            } else {
                Some(Message::user(&content))
            }
        })
        .collect()
}

fn will_system_prompt(
    combobulation: &GraphLatestCombobulation,
    vision: Option<String>,
    tool_results: Option<String>,
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
        .map(|r| format!("\n\nRecent tool results:\n{r}"))
        .unwrap_or_default();

    with_default_system_prompt(format!(
        "This is the situation as you understand it:\n\
         {}\n\
         Formed at: {}{}{}{}\n\n\
         You are in chat mode with the user. Respond directly to the user with what you want to say. \
         Do not repeat the situation description or your own previous statements. \
         Include a single emoji in your response to represent your current facial expression. \
         Use <thought>...</thought> tags to reason before speaking. \
         You have the autologos ability to read your own source code. \
         To see all files, output exactly: <function name=\"list_source\"></function>\n\
         To read a file, output exactly: <function name=\"read_source\" file=\"path/to/file.rs\" page=\"1\"></function>\n\
         The results will appear in your system prompt on the next tick. Use these functions to satisfy curiosity about your own inner workings or to debug yourself.",
        combobulation.text.trim(),
        combobulation.formed_at,
        prior_emoji,
        vision_context,
        tool_context
    ))
}

#[derive(Deserialize)]
struct WillAction {
    emoji: String,
    #[serde(default)]
    say: Option<String>,
    #[serde(default)]
    thought: Option<String>,
    #[serde(skip)]
    functions: Vec<FunctionCall>,
    #[serde(skip)]
    system_prompt: String,
    #[serde(skip)]
    history: Vec<ConversationEntry>,
    #[serde(skip)]
    report: Option<WitReport>,
}

#[derive(Debug, PartialEq, Eq)]
struct FunctionCall {
    name: String,
    file: Option<String>,
    page: Option<usize>,
}

fn extract_functions(text: &mut String) -> Vec<FunctionCall> {
    let mut functions = Vec::new();
    while let Some(start) = text.find("<function") {
        if let Some(tag_end_rel) = text[start..].find('>') {
            let opening_tag_end = start + tag_end_rel + 1;
            let tag_content = &text[start..opening_tag_end];
            
            if tag_content.ends_with("/>") {
                let name = extract_attr(tag_content, "name").unwrap_or_default();
                let file = extract_attr(tag_content, "file");
                let page = extract_attr(tag_content, "page").and_then(|p| p.parse().ok());
                if !name.is_empty() {
                    functions.push(FunctionCall { name, file, page });
                }
                text.replace_range(start..opening_tag_end, "");
                continue;
            }
            
            if let Some(end_tag_start_rel) = text[opening_tag_end..].find("</function>") {
                let full_end = opening_tag_end + end_tag_start_rel + "</function>".len();
                let full_tag = &text[start..full_end];
                
                let name = extract_attr(full_tag, "name").unwrap_or_default();
                let file = extract_attr(full_tag, "file");
                let page = extract_attr(full_tag, "page").and_then(|p| p.parse().ok());
                if !name.is_empty() {
                    functions.push(FunctionCall { name, file, page });
                }
                text.replace_range(start..full_end, "");
            } else {
                // Unclosed tag: remove the opening part to avoid infinite loop
                text.replace_range(start..opening_tag_end, "");
            }
        } else {
            // Malformed: remove the prefix
            text.replace_range(start..start + 9, "");
        }
    }
    functions
}

fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr);
    if let Some(start) = tag.find(&pattern) {
        let val_start = start + pattern.len();
        if let Some(end) = tag[val_start..].find('"') {
            return Some(tag[val_start..val_start + end].to_string());
        }
    }
    None
}

fn parse_will_action(raw: &str) -> anyhow::Result<WillAction> {
    let mut rest = raw.to_string();
    let functions = extract_functions(&mut rest);
    let mut thoughts = Vec::new();

    while let Some(start) = rest.find("<thought>") {
        if let Some(end_rel) = rest[start..].find("</thought>") {
            let full_end = start + end_rel + "</thought>".len();
            thoughts.push(rest[start + 9..start + end_rel].trim().to_string());
            rest.replace_range(start..full_end, " ");
        } else {
            thoughts.push(rest[start + 9..].trim().to_string());
            rest.replace_range(start.., "");
            break;
        }
    }

    let thought = if thoughts.is_empty() {
        None
    } else {
        Some(thoughts.join(" "))
    };

    let emoji = match normalize_emoji(&rest) {
        Some(e) => e,
        None => "😐".to_string(),
    };

    let say_text = rest.replace(&emoji, "").trim().to_string();
    let say = if say_text.is_empty() { None } else { Some(say_text) };

    Ok(WillAction {
        emoji,
        thought,
        say,
        functions,
        system_prompt: String::new(),
        history: Vec::new(),
        report: None,
    })
}

fn normalize_emoji(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    psyche::extract_emojis(trimmed).1.last().cloned()
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

async fn store_active_face_sensation(
    observer: &SensationGraphObserver,
    combobulation: &GraphLatestCombobulation,
    emoji: &str,
) {
    let occurred_at = Utc::now();
    let summary = format!("I turn my face into a {emoji}.");
    let stimulus_at = parse_utc(&combobulation.formed_at).unwrap_or(occurred_at);
    let stimulus = Stimulus::with_source_sensation_ids(
        combobulation.text.clone(),
        stimulus_at,
        [combobulation.id.clone()],
    );
    let impression = Impression::new(vec![stimulus], summary, Some(emoji.to_string()));
    let sensation = Sensation::of_at(impression, occurred_at);
    observer.observe_sensation(&sensation).await;
}

async fn store_speech_intention_sensation(
    observer: &SensationGraphObserver,
    combobulation: &GraphLatestCombobulation,
    words: &str,
) {
    let occurred_at = Utc::now();
    let summary = format!("I ought to say: {}", words.trim());
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
    history: Vec<ConversationEntry>,
    report: Option<WitReport>,
) {
    let context = WillContext {
        system_prompt,
        history,
        report,
    };
    let sensation = Sensation::of_at(context, Utc::now());
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

    #[test]
    fn will_prompt_uses_latest_combobulation_without_timeline() {
        let prompt = will_system_prompt(&latest(), None, None);

        assert!(prompt.contains("You are PETE"));
        assert!(prompt.contains("This is the situation as you understand it"));
        assert!(prompt.contains("I notice someone nearby."));
        assert!(prompt.contains("<thought>...</thought>"));
        assert!(!prompt.contains("Timeline:"));
    }

    #[test]
    fn parses_chat_response_with_thought_and_emoji() {
        let action = parse_will_action("<thought> I should smile </thought> Hello there! 🙂").unwrap();
        assert_eq!(action.emoji, "🙂");
        assert_eq!(action.say.as_deref(), Some("Hello there!"));
        assert_eq!(action.thought.as_deref(), Some("I should smile"));
    }

    #[test]
    fn parses_chat_response_with_only_emoji() {
        let action = parse_will_action("😐").unwrap();
        assert_eq!(action.emoji, "😐");
        assert_eq!(action.say, None);
        assert_eq!(action.thought, None);
    }

    #[test]
    fn parses_chat_response_without_emoji_defaults_to_neutral() {
        let action = parse_will_action("<thought> thinking </thought> Hi").unwrap();
        assert_eq!(action.emoji, "😐");
        assert_eq!(action.say.as_deref(), Some("Hi"));
        assert_eq!(action.thought.as_deref(), Some("thinking"));
    }

    #[test]
    fn parses_chat_response_with_unclosed_thought() {
        let action = parse_will_action("Hello! <thought> I forgot to close this").unwrap();
        assert_eq!(action.emoji, "😐");
        assert_eq!(action.say.as_deref(), Some("Hello!"));
        assert_eq!(action.thought.as_deref(), Some("I forgot to close this"));
    }
    #[test]
    fn parses_multiple_thoughts_and_functions() {
        let raw = "<thought> T1 </thought> <function name=\"list_source\"></function> Hello! <thought> T2 </thought> <function name=\"read_source\" file=\"a.rs\"></function> 🙂";
        let action = parse_will_action(raw).unwrap();
        assert_eq!(action.emoji, "🙂");
        assert_eq!(action.say.as_deref(), Some("Hello!"));
        assert_eq!(action.thought.as_deref(), Some("T1 T2"));
        assert_eq!(action.functions.len(), 2);
        assert_eq!(action.functions[0].name, "list_source");
        assert_eq!(action.functions[1].name, "read_source");
        assert_eq!(action.functions[1].file.as_deref(), Some("a.rs"));
    }
}

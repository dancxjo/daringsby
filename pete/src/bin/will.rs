use std::sync::OnceLock;
use std::time::Duration;

use anyhow::Context;
use chrono::{DateTime, Utc};
use clap::Parser;
use dotenvy::dotenv;
use lingproc::{Doer, LlmInstruction};
use pete::{EventBus, init_logging, ollama_provider_from_args};
use psyche::{
    ConversationEntry, GraphLatestCombobulation, Impression, Neo4jClient, Sensation,
    SensationGraphObserver, SensationObserver, Stimulus, WillContext, WitReport,
    with_default_system_prompt,
};
use serde::{Deserialize, Serialize};
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
    let processor = WillProcessor {
        doer,
        graph: graph.clone(),
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
            JavascriptCommand::ListFiles => {
                let summary = execute_list_files();
                store_function_result_sensation(observer, &combobulation, "list_files", &summary)
                    .await;
            }
            JavascriptCommand::ReadSourceFile { file, page } => {
                let summary = execute_read_source_file(file, *page);
                store_function_result_sensation(
                    observer,
                    &combobulation,
                    "read_source_file",
                    &summary,
                )
                .await;
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

        let system_prompt = will_instruction_prompt(combobulation, vision, tool_context);
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
}

fn will_instruction_prompt(
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
        .map(|r| format!("\n\nRecent function results:\n{r}"))
        .unwrap_or_default();

    with_default_system_prompt(format!(
        "This is the situation as you understand it:\n\
         {}\n\
         Formed at: {}{}{}{}\n\n\
         You are the Will. Decide what the system should work on next. \
         You are not the conversation manager; do not answer the user conversationally unless you intentionally queue speech with say(text). \
         Return only a JSON object with exactly these fields:\n\
         {{\"thought\":\"one concise sentence about what the system should work on next\",\"javascript\":\"a short program using only the allowed functions\"}}\n\n\
         The javascript field may call only these functions:\n\
         say(text) - inserts speech into the queue.\n\
         list_files() - lists the extant source files.\n\
         read_source_file(path, page) - reads one source file page; page is optional and defaults to 1.\n\n\
         Use an empty string for javascript when no action is needed, but do try to keep yourself busy and prevent yourself from being idle. \
         You may call other functions, define functions, or import modules, but do not use markdown, or include text outside the JSON object. \
         Seek to understand the world around you and improve your own system.",
        combobulation.text.trim(),
        combobulation.formed_at,
        prior_emoji,
        vision_context,
        tool_context
    ))
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
    Say { text: String },
    ListFiles,
    ReadSourceFile { file: String, page: Option<usize> },
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
function say(text) {{
  __daringsbyCommands.push({{kind: "say", text: __daringsbyString(text)}});
}}
function list_files() {{
  __daringsbyCommands.push({{kind: "list_files"}});
}}
function read_source_file(path, page) {{
  const command = {{kind: "read_source_file", file: __daringsbyString(path)}};
  if (page !== undefined) command.page = Number(page);
  __daringsbyCommands.push(command);
}}
const read_file = read_source_file;
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

    #[test]
    fn will_prompt_uses_instruction_json_without_conversation() {
        let prompt = will_instruction_prompt(&latest(), None, None);

        assert!(prompt.contains("You are PETE"));
        assert!(prompt.contains("Return only a JSON object"));
        assert!(prompt.contains("\"thought\""));
        assert!(prompt.contains("\"javascript\""));
        assert!(prompt.contains("list_files()"));
        assert!(prompt.contains("read_source_file(path, page)"));
        assert!(!prompt.contains("conversation history"));
        assert!(!prompt.contains("<thought>"));
        assert!(!prompt.contains("<function"));
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

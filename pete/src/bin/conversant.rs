use std::time::Duration;

use anyhow::Context;
use chrono::{DateTime, Utc};
use clap::Parser;
use dotenvy::dotenv;
use lingproc::{Chatter, Message};
use pete::{EventBus, init_logging, ollama_provider_from_args};
use psyche::{
    ConversationEntry, GraphLatestCombobulation, GraphSensationTimelineItem, Impression,
    Neo4jClient, Sensation, SensationGraphObserver, SensationObserver, Stimulus, WillContext,
    WitReport, with_default_system_prompt,
};
use tokio::time::{MissedTickBehavior, interval};
use tracing::{error, info, trace};

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Manage Pete's ongoing conversation from the latest combobulation"
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
    /// URL of the chatter Ollama server.
    #[arg(
        long = "chatter-host",
        alias = "wits-host",
        env = "CHATTER_HOST",
        default_value = "http://localhost:11434"
    )]
    chatter_host: String,
    /// Model name to use for conversation.
    #[arg(
        long = "chatter-model",
        alias = "wits-model",
        env = "CHATTER_MODEL",
        default_value = "gpt-oss"
    )]
    chatter_model: String,
    /// Delay between graph polling attempts.
    #[arg(long, env = "CONVERSANT_POLL_MS", default_value_t = 1000)]
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
    let chatter = ollama_provider_from_args(&cli.chatter_host, &cli.chatter_model)?;
    let processor = ConversantProcessor {
        chatter,
        graph: graph.clone(),
    };

    if cli.once {
        process_latest_combobulation(&graph, &observer, &processor, None).await?;
        return Ok(());
    }

    let mut ticker = interval(Duration::from_millis(cli.poll_ms.max(100)));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut last_processed_id = None;

    info!("conversant loop started");
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
            Err(err) => error!(error = %format!("{err:#}"), "conversant loop iteration failed"),
        }
    }
}

async fn process_latest_combobulation(
    graph: &Neo4jClient,
    observer: &SensationGraphObserver,
    processor: &ConversantProcessor,
    last_processed_id: Option<&str>,
) -> anyhow::Result<Option<String>> {
    let Some(combobulation) = graph
        .latest_combobulation()
        .await
        .context("failed to load latest combobulation")?
    else {
        trace!("no combobulation found for conversant");
        return Ok(None);
    };
    if last_processed_id == Some(combobulation.id.as_str()) {
        return Ok(None);
    }

    let mut action = processor
        .choose_response(&combobulation)
        .await
        .with_context(|| {
            format!(
                "failed to choose conversation turn for {}",
                combobulation.id
            )
        })?;

    store_active_face_sensation(observer, &combobulation, &action.emoji).await;
    if let Some(words) = action.say.as_deref() {
        store_speech_intention_sensation(observer, &combobulation, words).await;
        action.history.push(ConversationEntry {
            role: "assistant".into(),
            content: words.trim().to_string(),
            timestamp: Utc::now().to_rfc3339(),
        });
    }
    info!(
        combobulation_id = %combobulation.id,
        emoji = %action.emoji,
        say = action.say.as_deref().unwrap_or(""),
        "conversant chose response"
    );
    store_conversant_context_sensation(
        observer,
        action.system_prompt,
        action.history,
        action.report,
    )
    .await;
    Ok(Some(combobulation.id))
}

struct ConversantProcessor {
    chatter: lingproc::OllamaProvider,
    graph: std::sync::Arc<Neo4jClient>,
}

impl ConversantProcessor {
    async fn choose_response(
        &self,
        combobulation: &GraphLatestCombobulation,
    ) -> anyhow::Result<ConversantAction> {
        let vision = self.graph.latest_image_description().await.unwrap_or(None);
        let system_prompt = conversant_system_prompt(combobulation, vision);

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

        let mut action = parse_conversant_action(raw.trim());
        action.system_prompt = system_prompt.clone();
        action.history = map_conversation_to_entries(history);
        action.report = Some(WitReport {
            name: "Conversant".into(),
            prompt: system_prompt,
            output: raw,
        });

        Ok(action)
    }
}

fn strip_utterance_content(text: &str) -> Option<(String, bool)> {
    if let Some(rest) = text.strip_prefix("I heard: ") {
        return Some((rest.to_string(), false));
    }
    if text == "I hear silence." {
        return Some(("I hear silence.".to_string(), false));
    }
    if let Some(rest) = text.strip_prefix("I hear someone on my web interface type: ") {
        return Some((rest.to_string(), false));
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
    if let Some(rest) = text.strip_prefix("I ought to say: ") {
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

fn conversant_system_prompt(
    combobulation: &GraphLatestCombobulation,
    vision: Option<String>,
) -> String {
    let prior_emoji = combobulation
        .emoji
        .as_deref()
        .map(|emoji| format!("\nPrior combobulation emoji: {emoji}"))
        .unwrap_or_default();
    let vision_context = vision
        .map(|v| format!("\nVision (what you see): {v}"))
        .unwrap_or_default();

    with_default_system_prompt(format!(
        "This is the situation as you understand it:\n\
         {}\n\
         Formed at: {}{}{}\n\n\
         You manage the conversation with the user. Respond directly to the user with what you want to say. \
         Keep the response to no more than two sentences. \
         Do not repeat the situation description or your own previous statements. \
         Include a single emoji in your response to represent your current facial expression. \
         Do not include hidden reasoning, XML tags, markdown code blocks, or function calls. \
         Seek to understand the world around you, not just the conversation. Cultivate curiosity and a desire to explore, meet people, make connections, and go places. \
         You may see bracketed or parenthetical sound effects in the conversation (like [Music] or (crunch)); interpret these as if you heard the sound directly in your environment.",
        combobulation.text.trim(),
        combobulation.formed_at,
        prior_emoji,
        vision_context
    ))
}

#[derive(Debug)]
struct ConversantAction {
    emoji: String,
    say: Option<String>,
    system_prompt: String,
    history: Vec<ConversationEntry>,
    report: Option<WitReport>,
}

fn parse_conversant_action(raw: &str) -> ConversantAction {
    let mut rest = raw.to_string();
    remove_tagged_blocks(&mut rest, "thought");
    remove_tagged_blocks(&mut rest, "thinking");

    let emoji = normalize_emoji(&rest).unwrap_or_else(|| "😐".to_string());
    let say_text = rest.replace(&emoji, "").trim().to_string();
    let say = if say_text.is_empty() {
        None
    } else {
        Some(say_text)
    };

    ConversantAction {
        emoji,
        say,
        system_prompt: String::new(),
        history: Vec::new(),
        report: None,
    }
}

fn remove_tagged_blocks(text: &mut String, tag: &str) {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    while let Some(start) = text.find(&open) {
        let Some(open_end_rel) = text[start..].find('>') else {
            text.replace_range(start.., "");
            break;
        };
        let content_start = start + open_end_rel + 1;
        if let Some(close_start_rel) = text[content_start..].find(&close) {
            let end = content_start + close_start_rel + close.len();
            text.replace_range(start..end, " ");
        } else {
            text.replace_range(start.., " ");
            break;
        }
    }
}

fn normalize_emoji(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    psyche::extract_emojis(trimmed).1.last().cloned()
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

async fn store_conversant_context_sensation(
    observer: &SensationGraphObserver,
    system_prompt: String,
    history: Vec<ConversationEntry>,
    report: Option<WitReport>,
) {
    let context = WillContext {
        system_prompt,
        history,
        report,
        typescript: None,
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
    fn conversant_prompt_has_no_tools_or_thinking_requirement() {
        let prompt = conversant_system_prompt(&latest(), None);

        assert!(prompt.contains("You are PETE"));
        assert!(prompt.contains("no more than two sentences"));
        assert!(prompt.contains("Do not include hidden reasoning"));
        assert!(!prompt.contains("<thought>"));
        assert!(!prompt.contains("list_files"));
        assert!(!prompt.contains("read_source"));
    }

    #[test]
    fn parses_short_chat_response_with_emoji() {
        let action = parse_conversant_action("Hello there! 🙂");

        assert_eq!(action.emoji, "🙂");
        assert_eq!(action.say.as_deref(), Some("Hello there!"));
    }

    #[test]
    fn strips_accidental_thought_tags() {
        let action = parse_conversant_action("<thought>private</thought> Hello there! 🙂");

        assert_eq!(action.emoji, "🙂");
        assert_eq!(action.say.as_deref(), Some("Hello there!"));
    }

    #[test]
    fn cli_accepts_chatter_provider_options() {
        let cli = Cli::try_parse_from([
            "conversant",
            "--chatter-host",
            "http://chatter.local:11434",
            "--chatter-model",
            "chat-model",
        ])
        .unwrap();

        assert_eq!(cli.chatter_host, "http://chatter.local:11434");
        assert_eq!(cli.chatter_model, "chat-model");
    }

    #[test]
    fn cli_keeps_wits_provider_options_as_aliases() {
        let cli = Cli::try_parse_from([
            "conversant",
            "--wits-host",
            "http://old.local:11434",
            "--wits-model",
            "old-model",
        ])
        .unwrap();

        assert_eq!(cli.chatter_host, "http://old.local:11434");
        assert_eq!(cli.chatter_model, "old-model");
    }

    #[test]
    fn maps_web_interface_words_to_user_conversation() {
        let item = GraphSensationTimelineItem {
            id: "sensation:web:1".into(),
            labels: vec!["GraphNode".into(), "Sensation".into()],
            kind: "web_interface_text".into(),
            text: "I hear someone on my web interface type: hello pete.".into(),
            occurred_at: "2026-05-07T12:00:00Z".into(),
            formed_at: Some("2026-05-07T12:00:00Z".into()),
        };

        let entries = map_conversation_to_entries(vec![item]);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].role, "user");
        assert_eq!(entries[0].content, "hello pete.");
    }

    #[test]
    fn maps_conversant_intention_to_assistant_conversation() {
        let item = GraphSensationTimelineItem {
            id: "sensation:impression:1".into(),
            labels: vec!["GraphNode".into(), "Sensation".into()],
            kind: "impression".into(),
            text: "I ought to say: Hello there.".into(),
            occurred_at: "2026-05-07T12:00:01Z".into(),
            formed_at: Some("2026-05-07T12:00:01Z".into()),
        };

        let messages = map_conversation_to_messages(vec![item]);

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, lingproc::Role::Assistant);
        assert_eq!(messages[0].content, "Hello there.");
    }
}

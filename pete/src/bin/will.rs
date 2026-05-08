use std::time::Duration;

use anyhow::Context;
use chrono::{DateTime, Utc};
use clap::Parser;
use dotenvy::dotenv;
use lingproc::{Doer, LlmInstruction};
use pete::{EventBus, init_logging, ollama_provider_from_args};
use psyche::{
    GraphLatestCombobulation, Impression, Neo4jClient, Sensation, SensationGraphObserver,
    SensationObserver, Stimulus, with_default_system_prompt,
};
use serde::Deserialize;
use tokio::time::{MissedTickBehavior, interval};
use tracing::{error, info, trace};

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
    let processor = WillProcessor { doer };

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

    let emoji = processor
        .choose_emoji(&combobulation)
        .await
        .with_context(|| format!("failed to choose face expression for {}", combobulation.id))?;
    store_active_face_sensation(observer, &combobulation, &emoji).await;
    info!(
        combobulation_id = %combobulation.id,
        %emoji,
        "will chose active face expression"
    );
    Ok(Some(combobulation.id))
}

struct WillProcessor {
    doer: lingproc::OllamaProvider,
}

impl WillProcessor {
    async fn choose_emoji(
        &self,
        combobulation: &GraphLatestCombobulation,
    ) -> anyhow::Result<String> {
        let prompt = will_prompt(combobulation);
        let raw = self
            .doer
            .follow(LlmInstruction {
                command: prompt,
                images: Vec::new(),
            })
            .await?
            .trim()
            .to_string();
        parse_will_emoji(&raw)
    }
}

fn will_prompt(combobulation: &GraphLatestCombobulation) -> String {
    let prior_emoji = combobulation
        .emoji
        .as_deref()
        .map(|emoji| format!("\nPrior combobulation emoji: {emoji}"))
        .unwrap_or_default();
    with_default_system_prompt(format!(
        "Here is your latest combobulation about your most recent experiences. This is not a timeline; use it as the current situation summary.\n\n\
         Latest combobulation:\n{}\n\
         Formed at: {}{}\n\n\
         Choose the single emoji your face may actively present to the world right now. Return only strict JSON with this shape:\n\
         {{\"emoji\":\"🙂\"}}",
        combobulation.text.trim(),
        combobulation.formed_at,
        prior_emoji
    ))
}

#[derive(Deserialize)]
struct WillEmoji {
    emoji: String,
}

fn parse_will_emoji(raw: &str) -> anyhow::Result<String> {
    if let Ok(parsed) = serde_json::from_str::<WillEmoji>(raw) {
        if let Some(emoji) = normalize_emoji(&parsed.emoji) {
            return Ok(emoji);
        }
    }
    if let Some(json) = first_json_object(raw) {
        if let Ok(parsed) = serde_json::from_str::<WillEmoji>(&json) {
            if let Some(emoji) = normalize_emoji(&parsed.emoji) {
                return Ok(emoji);
            }
        }
    }
    if let Some(emoji) = normalize_emoji(raw) {
        return Ok(emoji);
    }
    anyhow::bail!("will response did not contain an emoji");
}

fn normalize_emoji(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    psyche::extract_emojis(trimmed).1.last().cloned()
}

fn first_json_object(raw: &str) -> Option<String> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    (start <= end).then(|| raw[start..=end].to_string())
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
        let prompt = will_prompt(&latest());

        assert!(prompt.contains("You are PETE"));
        assert!(prompt.contains("latest combobulation about your most recent experiences"));
        assert!(prompt.contains("This is not a timeline"));
        assert!(prompt.contains("I notice someone nearby."));
        assert!(prompt.contains("{\"emoji\":\"🙂\"}"));
        assert!(!prompt.contains("Timeline:"));
    }

    #[test]
    fn parses_strict_json_emoji() {
        assert_eq!(parse_will_emoji(r#"{"emoji":"😐"}"#).unwrap(), "😐");
    }

    #[test]
    fn parses_json_object_inside_model_chatter() {
        assert_eq!(
            parse_will_emoji("Here:\n```json\n{\"emoji\":\"🙂\"}\n```").unwrap(),
            "🙂"
        );
    }
}

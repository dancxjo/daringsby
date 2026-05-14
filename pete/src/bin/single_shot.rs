use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use chrono::{DateTime, Utc};
use clap::Parser;
use dotenvy::dotenv;
use lingproc::{Doer, ImageData as LImageData, LlmInstruction, Vectorizer};
use pete::{EventBus, init_logging, ollama_provider_from_args};
use psyche::{
    BasicMemory, CONVERSATION_SPEAKER_NOTE, ConversationEntry, GraphAwareness,
    GraphImageDescription, GraphImageFrame, GraphLatestCombobulation, GraphSensationTimelineItem,
    GraphTimelineWindow, IMAGE_CAPTION_PROMPT, Impression, Memory, Neo4jClient, QdrantClient,
    SENSOR_GROUNDING_RULES, Sensation, SensationGraphObserver, SensationObserver, Stimulus,
    Thought, WillTypeScriptExecution, WillTypeScriptResult, WitReport, with_default_system_prompt,
};
use serde::Deserialize;
use tokio::time::{MissedTickBehavior, interval};
use tracing::{debug, error, info, trace, warn};

const DEFAULT_SINGLE_SHOT_MODEL: &str = "gemma4";

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Run Pete's vision, combobulation, will, and conversant cycle in one multimodal LLM shot"
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
    /// URL of the multimodal Ollama server.
    #[arg(
        long = "single-shot-host",
        alias = "wits-host",
        env = "SINGLE_SHOT_HOST",
        default_value = "http://localhost:11434"
    )]
    single_shot_host: String,
    /// Vision-capable model for the single-shot cognition pass.
    #[arg(
        long = "single-shot-model",
        alias = "wits-model",
        env = "SINGLE_SHOT_MODEL",
        default_value = DEFAULT_SINGLE_SHOT_MODEL
    )]
    single_shot_model: String,
    /// URL of the embeddings Ollama server.
    #[arg(
        long,
        env = "EMBEDDINGS_HOST",
        default_value = "http://localhost:11434"
    )]
    embeddings_host: String,
    /// Model name to use for awareness and image-description embeddings.
    #[arg(long, env = "EMBEDDINGS_MODEL", default_value = "embeddinggemma")]
    embeddings_model: String,
    /// Number of seconds of graph history to include in one FIFO cognition chunk.
    #[arg(long, env = "SINGLE_SHOT_WINDOW_SECONDS", default_value_t = 600)]
    window_seconds: u64,
    /// Maximum timeline items to include in one LLM prompt; 0 includes all sensations in the window.
    #[arg(long, env = "SINGLE_SHOT_WINDOW_LIMIT", default_value_t = 0)]
    window_limit: usize,
    /// Delay between graph polling attempts.
    #[arg(long, env = "SINGLE_SHOT_POLL_MS", default_value_t = 250)]
    poll_ms: u64,
    /// Process at most one pending timeline window and exit.
    #[arg(long)]
    once: bool,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let (bus, _user_rx) = EventBus::new();
    init_logging(bus.log_sender());
    dotenv().ok();

    let cli = Cli::parse();
    ensure_vision_model(&cli.single_shot_model)?;

    let graph = Arc::new(Neo4jClient::new(
        cli.neo4j_uri.clone(),
        cli.neo4j_user.clone(),
        cli.neo4j_pass.clone(),
    ));
    let observer = SensationGraphObserver::new(graph.clone());
    let qdrant = QdrantClient::new(cli.qdrant_url.clone());
    let doer = ollama_provider_from_args(&cli.single_shot_host, &cli.single_shot_model)?;
    let vectorizer = ollama_provider_from_args(&cli.embeddings_host, &cli.embeddings_model)?;
    let memory: Arc<dyn Memory> = Arc::new(BasicMemory {
        vectorizer: Arc::new(vectorizer.clone()),
        qdrant: qdrant.clone(),
        neo4j: graph.clone(),
    });
    let processor = SingleShotProcessor {
        doer,
        vectorizer,
        llm_model: cli.single_shot_model,
        embedding_model: cli.embeddings_model,
        memory,
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
        "single-shot cognition loop started"
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
            error!(error = %format!("{err:#}"), "single-shot cognition loop iteration failed");
        }
    }
}

async fn process_next_window(
    graph: &Neo4jClient,
    qdrant: &QdrantClient,
    observer: &SensationGraphObserver,
    processor: &SingleShotProcessor,
    window_seconds: u64,
    window_limit: usize,
) -> anyhow::Result<Option<String>> {
    let Some(window) = graph
        .latest_timeline_window_for_combobulation(window_seconds, window_limit)
        .await
        .context("failed to load next timeline window")?
    else {
        trace!("no timeline windows found for single-shot cognition");
        return Ok(None);
    };
    if window.items.is_empty() {
        debug!(
            anchor_id = %window.anchor_id,
            "timeline window had no source events"
        );
        return Ok(None);
    }

    let latest_combobulation_sensation_at = graph
        .latest_combobulation_sensation_at()
        .await
        .context("failed to load latest combobulation sensation timestamp")?;
    let conversation = graph
        .conversation_timeline(None, Utc::now(), 20)
        .await
        .unwrap_or_default();
    let tool_results = graph.latest_function_results(3).await.unwrap_or_default();
    let latest_image = graph
        .latest_image_frame()
        .await
        .context("failed to load latest image frame")?;
    let image_needing_description = graph
        .latest_unprocessed_image_frame_for_description()
        .await
        .context("failed to load latest undescribed image frame")?;
    let should_attach_image_description = latest_image
        .as_ref()
        .zip(image_needing_description.as_ref())
        .is_some_and(|(latest, undescribed)| latest.id == undescribed.id);

    info!(
        anchor_id = %window.anchor_id,
        source_count = window.items.len(),
        image_id = latest_image.as_ref().map(|frame| frame.id.as_str()).unwrap_or(""),
        "running single-shot cognition"
    );
    info!(target: "thought_stream", "timeline:\n{}", timeline_prompt(&window));

    let result = processor
        .run(
            &window,
            latest_combobulation_sensation_at.as_deref(),
            &conversation,
            &tool_results,
            latest_image.as_ref(),
            should_attach_image_description,
            qdrant,
            window_seconds,
        )
        .await
        .with_context(|| {
            format!(
                "failed to run single-shot cognition for {}",
                window.anchor_id
            )
        })?;

    graph
        .attach_combobulation(
            &window,
            &processor.llm_model,
            &processor.embedding_model,
            &result.awareness,
        )
        .await
        .with_context(|| {
            format!(
                "failed to attach combobulation for timeline {}",
                window.anchor_id
            )
        })?;

    if let (true, Some(frame), Some(description)) = (
        should_attach_image_description,
        &latest_image,
        &result.image_description,
    ) {
        graph
            .attach_image_description(
                frame,
                &processor.llm_model,
                &processor.embedding_model,
                description,
            )
            .await
            .with_context(|| format!("failed to attach image description for {}", frame.id))?;
    }

    let combobulation = GraphLatestCombobulation {
        id: result.awareness.awareness_id.clone(),
        text: result.awareness.text.clone(),
        emoji: result.awareness.emoji.clone(),
        formed_at: Utc::now().to_rfc3339(),
    };

    if !result.action.thought.trim().is_empty() {
        store_memory_impression(
            observer,
            processor.memory.as_ref(),
            &combobulation,
            format!("I think: {}", result.action.thought.trim()),
            None,
        )
        .await;
        info!(target: "thought_stream", "think: {}", result.action.thought.trim());
    }
    for note in &result.action.notes {
        store_memory_impression(
            observer,
            processor.memory.as_ref(),
            &combobulation,
            format!("I note: {}", note.trim()),
            None,
        )
        .await;
        info!(target: "thought_stream", "note: {}", note.trim());
    }
    for memory in &result.action.remember {
        store_memory_impression(
            observer,
            processor.memory.as_ref(),
            &combobulation,
            format!("I remember: {}", memory.trim()),
            None,
        )
        .await;
        info!(target: "thought_stream", "remember: {}", memory.trim());
    }
    if let Some(emoji) = result.awareness.emoji.as_deref() {
        store_face_expression_sensation(observer, &combobulation, emoji).await;
        info!(target: "thought_stream", "face: {}", emoji.trim());
    }
    if let Some(words) = result.action.say.as_deref() {
        store_speech_intention_sensation(observer, &combobulation, words).await;
        info!(target: "thought_stream", "say: {}", words.trim());
    }

    store_single_shot_context_sensation(
        observer,
        &window,
        latest_image.as_ref(),
        result.prompt,
        result.report,
        result.action,
    )
    .await;

    info!(
        anchor_id = %window.anchor_id,
        awareness_id = %result.awareness.awareness_id,
        "single-shot cognition attached graph outputs"
    );
    Ok(Some(window.anchor_id))
}

struct SingleShotProcessor {
    doer: lingproc::OllamaProvider,
    vectorizer: lingproc::OllamaProvider,
    llm_model: String,
    embedding_model: String,
    memory: Arc<dyn Memory>,
}

struct SingleShotResult {
    awareness: GraphAwareness,
    image_description: Option<GraphImageDescription>,
    action: SingleShotAction,
    prompt: String,
    report: WitReport,
}

impl SingleShotProcessor {
    async fn run(
        &self,
        window: &GraphTimelineWindow,
        latest_combobulation_sensation_at: Option<&str>,
        conversation: &[GraphSensationTimelineItem],
        tool_results: &[String],
        latest_image: Option<&GraphImageFrame>,
        should_store_image_description: bool,
        qdrant: &QdrantClient,
        window_seconds: u64,
    ) -> anyhow::Result<SingleShotResult> {
        let prompt = single_shot_prompt(
            window,
            window_seconds,
            latest_combobulation_sensation_at,
            conversation,
            tool_results,
            latest_image,
        );
        let images = latest_image
            .map(llm_image_from_frame)
            .transpose()?
            .into_iter()
            .collect();
        let raw = self
            .doer
            .follow(LlmInstruction {
                command: prompt.clone(),
                images,
            })
            .await?
            .trim()
            .to_string();
        let action = parse_single_shot_action(&raw)?;

        let awareness_text =
            common::non_empty_model_text(&action.situation).context("empty situation")?;
        let awareness_embedding = self
            .vectorizer
            .vectorize(awareness_text)
            .await
            .context("failed to embed awareness text")?;
        anyhow::ensure!(
            !awareness_embedding.is_empty(),
            "embedding model returned no vector for timeline {}",
            window.anchor_id
        );
        let awareness_id = awareness_id(window);
        let awareness_vector_id = qdrant
            .store_vector_for_node(awareness_text, Some(&awareness_id), &awareness_embedding)
            .await
            .context("failed to store awareness vector")?
            .to_string();

        let image_description = if let (true, Some(frame), Some(text)) = (
            should_store_image_description,
            latest_image,
            common::non_empty_model_text(&action.image_description),
        ) {
            let embedding = self
                .vectorizer
                .vectorize(text)
                .await
                .context("failed to embed image description")?;
            anyhow::ensure!(
                !embedding.is_empty(),
                "embedding model returned no vector for image {}",
                frame.id
            );
            let description_id = image_description_id(&frame.id);
            let mut related = vec![frame.id.as_str()];
            if let Some(sensation_id) = &frame.sensation_id {
                related.push(sensation_id.as_str());
            }
            let vector_id = qdrant
                .store_image_description_vector_for_node_with_model(
                    &frame.id,
                    text,
                    &description_id,
                    &related,
                    Some(&self.embedding_model),
                    &embedding,
                )
                .await
                .context("failed to store image description vector")?
                .to_string();
            Some(GraphImageDescription {
                description_id,
                text: text.to_string(),
                vector_id,
                embedding_len: embedding.len(),
            })
        } else {
            None
        };

        let emoji = action
            .emoji
            .as_deref()
            .and_then(normalize_emoji)
            .or_else(|| psyche::extract_emojis(awareness_text).1.last().cloned());

        Ok(SingleShotResult {
            awareness: GraphAwareness {
                awareness_id,
                text: awareness_text.to_string(),
                emoji,
                vector_id: awareness_vector_id,
                embedding_len: awareness_embedding.len(),
            },
            image_description,
            action,
            prompt: prompt.clone(),
            report: WitReport {
                name: "SingleShot".into(),
                prompt,
                output: raw,
            },
        })
    }
}

#[derive(Debug, Default, Deserialize)]
struct SingleShotPayload {
    #[serde(default)]
    situation: String,
    #[serde(default)]
    image_description: String,
    #[serde(default)]
    thought: String,
    #[serde(default)]
    say: String,
    #[serde(default)]
    emoji: String,
    #[serde(default)]
    note: Vec<String>,
    #[serde(default)]
    remember: Vec<String>,
}

#[derive(Debug)]
struct SingleShotAction {
    situation: String,
    image_description: String,
    thought: String,
    say: Option<String>,
    emoji: Option<String>,
    notes: Vec<String>,
    remember: Vec<String>,
}

fn parse_single_shot_action(raw: &str) -> anyhow::Result<SingleShotAction> {
    let payload = if let Ok(payload) = serde_json::from_str::<SingleShotPayload>(raw) {
        payload
    } else if let Some(json) = extract_first_json_object(raw) {
        serde_json::from_str(json)?
    } else {
        let (text, emojis) = psyche::extract_emojis(raw);
        return Ok(SingleShotAction {
            situation: text,
            image_description: String::new(),
            thought: String::new(),
            say: None,
            emoji: emojis.last().cloned(),
            notes: Vec::new(),
            remember: Vec::new(),
        });
    };

    let situation = common::non_empty_model_text(&payload.situation)
        .unwrap_or_else(|| raw.trim())
        .to_string();
    Ok(SingleShotAction {
        situation,
        image_description: common::non_empty_model_text(&payload.image_description)
            .unwrap_or_default()
            .to_string(),
        thought: common::non_empty_model_text(&payload.thought)
            .unwrap_or_default()
            .to_string(),
        say: common::non_empty_model_text(&payload.say).map(str::to_string),
        emoji: common::non_empty_model_text(&payload.emoji).map(str::to_string),
        notes: payload
            .note
            .into_iter()
            .filter_map(|text| common::non_empty_model_text(&text).map(str::to_string))
            .collect(),
        remember: payload
            .remember
            .into_iter()
            .filter_map(|text| common::non_empty_model_text(&text).map(str::to_string))
            .collect(),
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

fn single_shot_prompt(
    window: &GraphTimelineWindow,
    window_seconds: u64,
    latest_combobulation_sensation_at: Option<&str>,
    conversation: &[GraphSensationTimelineItem],
    tool_results: &[String],
    latest_image: Option<&GraphImageFrame>,
) -> String {
    let timeline = timeline_prompt(window);
    let conversation_context = format_recent_items(conversation);
    let latest_combobulation_note = match latest_combobulation_sensation_at {
        Some(occurred_at) => {
            format!("The last recorded combobulation sensation occurred at {occurred_at}.")
        }
        None => "There is no recorded prior combobulation sensation.".to_string(),
    };
    let image_context = latest_image
        .map(|frame| {
            format!(
                "A latest camera image is attached. Image graph id: {}. Captured at: {}.",
                frame.id,
                frame
                    .image
                    .captured_at
                    .as_deref()
                    .or(frame.occurred_at.as_deref())
                    .unwrap_or("unknown")
            )
        })
        .unwrap_or_else(|| "No camera image is attached.".to_string());
    let tool_context = if tool_results.is_empty() {
        "(no recent function results)".to_string()
    } else {
        tool_results.join("\n")
    };

    with_default_system_prompt(format!(
        "This is Pete's single-shot cognition cycle. You are doing the work that used to be split across image_desc, combobulator, will, and conversant.\n\
         The entries below are a chronological timeline of the next unprocessed sensations, selected FIFO from the oldest pending sensation and bounded to {window_seconds} seconds. {latest_combobulation_note}\n\
         Treat the timeline as fragmentary evidence about the real situation, not as the topic to describe. {SENSOR_GROUNDING_RULES}\n\
         {IMAGE_CAPTION_PROMPT}\n\
         {image_context}\n\n\
         Current conversation:\n{CONVERSATION_SPEAKER_NOTE}\n{conversation_context}\n\n\
         Recent function results:\n{tool_context}\n\n\
         Timeline:\n{timeline}\n\n\
         Return only a JSON object with exactly these fields:\n\
         {{\"situation\":\"one or two grounded first-person sentences about what is going on now, explicitly integrating the attached image when present\",\"image_description\":\"a grounded first-person description of the attached image, or an empty string if no image is attached\",\"thought\":\"a concise internal decision about what Pete should do next and why\",\"say\":\"brief words to say to the user, or an empty string if speaking is not useful\",\"emoji\":\"exactly one face emoji\",\"note\":[\"optional private notes\"],\"remember\":[\"optional memories worth preserving\"]}}\n\
         Do not include hidden reasoning, markdown, XML tags, or text outside the JSON object. Do not say you are observing a timeline, graph, image file, sensor stream, or entries."
    ))
}

fn format_recent_items(items: &[GraphSensationTimelineItem]) -> String {
    if items.is_empty() {
        return "(no current conversation)".into();
    }
    items
        .iter()
        .map(|item| format!("- {} [{}]: {}", item.occurred_at, item.kind, item.text))
        .collect::<Vec<_>>()
        .join("\n")
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

    format!(
        "Sensation timeline {} to {}\n{}",
        timeline_timestamp(from),
        timeline_timestamp(to),
        entries.join("\n")
    )
}

fn timeline_timestamp(value: &str) -> String {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| psyche::model::localized_timestamp(timestamp.with_timezone(&Utc)))
        .unwrap_or_else(|_| value.to_string())
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

fn llm_image_from_frame(frame: &GraphImageFrame) -> anyhow::Result<LImageData> {
    if !frame.image.mime.to_ascii_lowercase().starts_with("image/") {
        bail!("unsupported image MIME type {}", frame.image.mime);
    }
    let (base64, image_bytes) = normalized_image_base64(&frame.image.base64)
        .with_context(|| format!("invalid base64 image payload for {}", frame.id))?;
    trace!(
        image_id = %frame.id,
        image_base64_len = base64.len(),
        image_bytes,
        "including latest image payload in single-shot request"
    );
    Ok(LImageData {
        mime: frame.image.mime.clone(),
        base64,
        captured_at: frame.image.captured_at.clone(),
    })
}

fn normalized_image_base64(value: &str) -> anyhow::Result<(String, usize)> {
    let base64 = value
        .split_once(',')
        .map_or_else(|| value.trim(), |(_, encoded)| encoded.trim())
        .to_string();
    let bytes = BASE64_STANDARD
        .decode(base64.as_bytes())
        .context("failed to decode base64 image")?;
    Ok((base64, bytes.len()))
}

fn ensure_vision_model(model: &str) -> anyhow::Result<()> {
    let normalized = model.to_ascii_lowercase();
    if normalized == "gpt-oss" || normalized.starts_with("gpt-oss:") {
        bail!(
            "SINGLE_SHOT_MODEL={model} is text-only in Ollama; use a vision-capable model like {DEFAULT_SINGLE_SHOT_MODEL}"
        );
    }
    if matches!(
        normalized.as_str(),
        "gemma3:270m" | "gemma3:1b" | "gemma4:270m" | "gemma4:1b"
    ) {
        bail!(
            "SINGLE_SHOT_MODEL={model} is text-only; use a vision-capable model like {DEFAULT_SINGLE_SHOT_MODEL}"
        );
    }
    if normalized.contains("llama3") || normalized.contains("qwen3") {
        warn!(
            %model,
            "single-shot model does not look vision-capable; Ollama may ignore attached images"
        );
    }
    Ok(())
}

fn awareness_id(window: &GraphTimelineWindow) -> String {
    format!(
        "awareness:{}:{}:{}",
        window.anchor_id,
        window.anchor_at,
        window.items.len()
    )
}

fn image_description_id(image_id: &str) -> String {
    format!("image-description-text:{image_id}")
}

fn normalize_emoji(value: &str) -> Option<String> {
    let (_, emojis) = psyche::extract_emojis(value.trim());
    emojis.last().cloned()
}

async fn store_memory_impression(
    observer: &SensationGraphObserver,
    memory: &(dyn Memory + 'static),
    combobulation: &GraphLatestCombobulation,
    summary: String,
    emoji: Option<String>,
) {
    let occurred_at = Utc::now();
    let stimulus_at = parse_utc(&combobulation.formed_at).unwrap_or(occurred_at);
    let stimulus = Stimulus::with_source_sensation_ids(
        combobulation.text.clone(),
        stimulus_at,
        [combobulation.id.clone()],
    );
    let impression = Impression::new(vec![stimulus], summary, emoji);
    let sensation = Sensation::of_at(impression.clone(), occurred_at);
    observer.observe_sensation(&sensation).await;
    if let Err(err) = memory.store_serializable(&impression).await {
        warn!(
            error = %format!("{err:#}"),
            summary = %impression.summary,
            "single-shot memory store failed"
        );
    }
}

async fn store_face_expression_sensation(
    observer: &SensationGraphObserver,
    combobulation: &GraphLatestCombobulation,
    emoji: &str,
) {
    let Some(emoji) = common::non_empty_model_text(emoji) else {
        return;
    };
    store_plain_impression(
        observer,
        combobulation,
        format!("I turn my face into a {}.", emoji.trim()),
        Some(emoji.trim().to_string()),
    )
    .await;
}

async fn store_speech_intention_sensation(
    observer: &SensationGraphObserver,
    combobulation: &GraphLatestCombobulation,
    words: &str,
) {
    let Some(words) = common::non_empty_model_text(words) else {
        return;
    };
    store_plain_impression(
        observer,
        combobulation,
        format!("I ought to say: {words}"),
        None,
    )
    .await;
}

async fn store_plain_impression(
    observer: &SensationGraphObserver,
    combobulation: &GraphLatestCombobulation,
    summary: String,
    emoji: Option<String>,
) {
    let occurred_at = Utc::now();
    let stimulus_at = parse_utc(&combobulation.formed_at).unwrap_or(occurred_at);
    let stimulus = Stimulus::with_source_sensation_ids(
        combobulation.text.clone(),
        stimulus_at,
        [combobulation.id.clone()],
    );
    let impression = Impression::new(vec![stimulus], summary, emoji);
    let sensation = Sensation::of_at(impression, occurred_at);
    observer.observe_sensation(&sensation).await;
}

async fn store_single_shot_context_sensation(
    observer: &SensationGraphObserver,
    window: &GraphTimelineWindow,
    latest_image: Option<&GraphImageFrame>,
    system_prompt: String,
    report: WitReport,
    action: SingleShotAction,
) {
    let mut source_sensation_ids = window
        .items
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    if let Some(sensation_id) = latest_image.and_then(|frame| frame.sensation_id.clone()) {
        if !source_sensation_ids.iter().any(|id| id == &sensation_id) {
            source_sensation_ids.push(sensation_id);
        }
    }
    let typescript = WillTypeScriptExecution {
        source: String::new(),
        timestamp: Utc::now().to_rfc3339(),
        results: vec![WillTypeScriptResult {
            command: "single_shot".into(),
            output: format!(
                "situation={} say={} emoji={}",
                action.situation.trim(),
                action.say.as_deref().unwrap_or(""),
                action.emoji.as_deref().unwrap_or("")
            ),
        }],
    };
    let context = Thought {
        system_prompt,
        history: Vec::<ConversationEntry>::new(),
        report: Some(report),
        typescript: Some(typescript),
        source_sensation_ids,
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
    use psyche::GraphTimelineItem;

    fn window() -> GraphTimelineWindow {
        GraphTimelineWindow {
            anchor_id: "sensation:audio:1".into(),
            anchor_at: "2026-05-05T12:34:56Z".into(),
            items: vec![GraphTimelineItem {
                id: "sensation:audio:1".into(),
                event_id: "audio:1".into(),
                labels: vec!["Sensation".into()],
                text: "I heard: hello".into(),
                occurred_at: "2026-05-05T12:34:56Z".into(),
            }],
        }
    }

    #[test]
    fn parses_single_shot_json() {
        let action = parse_single_shot_action(
            r#"{"situation":"I hear someone greet me.","image_description":"I see a room.","thought":"Answer briefly.","say":"Hello.","emoji":"🙂","note":["the room is bright"],"remember":["someone greeted me"]}"#,
        )
        .unwrap();

        assert_eq!(action.situation, "I hear someone greet me.");
        assert_eq!(action.image_description, "I see a room.");
        assert_eq!(action.say.as_deref(), Some("Hello."));
        assert_eq!(action.emoji.as_deref(), Some("🙂"));
        assert_eq!(action.notes, vec!["the room is bright"]);
        assert_eq!(action.remember, vec!["someone greeted me"]);
    }

    #[test]
    fn prompt_requests_structured_multimodal_cognition() {
        let prompt = single_shot_prompt(&window(), 600, None, &[], &[], None);

        assert!(prompt.contains("image_desc, combobulator, will, and conversant"));
        assert!(prompt.contains("Timeline:"));
        assert!(prompt.contains("Return only a JSON object"));
        assert!(prompt.contains("\"situation\""));
        assert!(prompt.contains("\"image_description\""));
        assert!(prompt.contains("\"thought\""));
        assert!(prompt.contains("\"say\""));
        assert!(prompt.contains("\"emoji\""));
        assert!(prompt.contains("No camera image is attached."));
    }

    #[test]
    fn ensure_vision_model_rejects_text_only_default() {
        let err = ensure_vision_model("gpt-oss").unwrap_err();

        assert!(err.to_string().contains("text-only"));
    }
}

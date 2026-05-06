//! Prompt building utilities for subagents.
//!
//! Each subagent constructs its LLM prompt via a dedicated struct
//! implementing [`PromptFragment`]. These helpers centralize prompt
//! wording so it can be tweaked consistently.

/// Common interface for constructing prompts.
pub trait PromptFragment {
    /// Build a prompt from `input`.
    fn build_prompt(&self, input: &str) -> String;
}

pub const IMAGE_CAPTION_PROMPT: &str = "Describe only what you see in a single sentence, in the first person. Do not interpret this as an image; interpret it as the machine's own live view. This is your own vision looking out: when looking out, one does not see oneself. Anyone you see is most likely someone you're looking at, not yourself, unless you're looking in a mirror.";

pub const SENSOR_GROUNDING_RULES: &str = "Describe the real-world scene or event, not the sensor stream. Do not summarize the amount, density, cadence, or mix of input modalities as if that were the situation. Repeated camera frames, repeated faces, image embeddings, pending audio clips, and heartbeats are usually evidence to compress or ignore, not events to report. If the evidence does not reveal what is happening, say that I cannot tell what is happening yet. Do not infer emotional tone or words like chaotic, intense, overwhelming, anxious, or ominous from sensor volume alone.";

/// Prompt builder for the `Voice` subagent.
#[derive(Clone, Default)]
pub struct VoicePrompt;

impl PromptFragment for VoicePrompt {
    fn build_prompt(&self, input: &str) -> String {
        input.to_string()
    }
}

/// Prompt builder for the `Will` subagent.
#[derive(Clone, Default)]
pub struct WillPrompt;

impl PromptFragment for WillPrompt {
    fn build_prompt(&self, input: &str) -> String {
        format!("In one or two short sentences, what should Pete do or say next?\n{input}")
    }
}

/// Prompt builder for the `Combobulator` subagent.
#[derive(Clone, Default)]
pub struct CombobulatorPrompt;

impl PromptFragment for CombobulatorPrompt {
    fn build_prompt(&self, input: &str) -> String {
        format!(
            "The following entries are a timestamped timeline of Pete's internal representations of sensations and real-world events happening around or to him. Treat them as fragmentary, possibly contradictory, fleeting evidence about the actual situation, not as the topic to describe. Try to infer what is going on in the real world from those fragments. Some entries may be your own prior combobulation summaries looping back in as sensations; treat those as provisional, possibly stale self-context, not as fresh external evidence. When related entries describe an audio recording and the transcription derived from it, treat them as one real-world event. {SENSOR_GROUNDING_RULES} Do not say that you are observing a timeline, recordings, entries, a previous summary, or a shift in conversation. Compress repeated or low-level records into the real-world gist; do not enumerate ids, hashes, timestamps, edges, or detections unless they are the point.\n\n\
             What is going on right now? Summarize Pete's current awareness in one or two grounded first-person sentences:\n{input}"
        )
    }
}

/// Prompt builder that injects recent context from the `TopicBus`.
#[derive(Clone)]
pub struct ContextualPrompt {
    identity: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    situation: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    moment: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    instant: std::sync::Arc<std::sync::Mutex<Option<String>>>,
}

impl ContextualPrompt {
    /// Create a new prompt builder subscribed to `bus`.
    pub fn new(bus: crate::topics::TopicBus) -> Self {
        use crate::topics::Topic;
        use futures::StreamExt;
        let identity = std::sync::Arc::new(std::sync::Mutex::new(None));
        let situation = std::sync::Arc::new(std::sync::Mutex::new(None));
        let moment = std::sync::Arc::new(std::sync::Mutex::new(None));
        let instant = std::sync::Arc::new(std::sync::Mutex::new(None));
        let subs = [
            (Topic::Identity, identity.clone()),
            (Topic::Situation, situation.clone()),
            (Topic::Moment, moment.clone()),
            (Topic::Instant, instant.clone()),
        ];
        for (topic, store) in subs.into_iter() {
            let b = bus.clone();
            let s = store.clone();
            tokio::spawn(async move {
                let stream = b.subscribe(topic);
                tokio::pin!(stream);
                while let Some(payload) = stream.next().await {
                    if let Ok(sval) = std::sync::Arc::downcast::<String>(payload.clone()) {
                        *s.lock().unwrap() = Some((*sval).clone());
                        continue;
                    }
                    if let Ok(imp) = std::sync::Arc::downcast::<crate::Impression<String>>(payload)
                    {
                        *s.lock().unwrap() = Some(imp.summary.clone());
                    }
                }
            });
        }
        Self {
            identity,
            situation,
            moment,
            instant,
        }
    }

    fn latest(store: &std::sync::Arc<std::sync::Mutex<Option<String>>>) -> String {
        store.lock().unwrap().clone().unwrap_or_default()
    }
}

impl PromptFragment for ContextualPrompt {
    fn build_prompt(&self, input: &str) -> String {
        let id = Self::latest(&self.identity);
        let sit = Self::latest(&self.situation);
        let mom = Self::latest(&self.moment);
        let ins = Self::latest(&self.instant);
        tracing::debug!(%id, %sit, %mom, %ins, "injecting context");
        format!(
            "Pete’s context:\nIdentity: {id}\nSituation: {sit}\nMoment: {mom}\nInstant: {ins}\n\nRespond in character:\n{input}"
        )
    }
}

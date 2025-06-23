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
        format!("Summarize Pete's current awareness in one or two sentences:\n{input}")
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
                let mut stream = b.subscribe(topic);
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

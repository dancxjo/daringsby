use crate::{Event, Mouth};
use lingproc::{Chatter, Message, SentenceSegmenter};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::broadcast;
use tokio::time::Duration;
use tokio_stream::StreamExt;
use tracing::{debug, info, warn};

pub struct Voice {
    chatter: Arc<dyn Chatter>,
    mouth: Arc<Mutex<Arc<dyn Mouth + Send + Sync>>>,
    events: broadcast::Sender<Event>,
    ready: AtomicBool,
    extra_prompt: Arc<Mutex<Option<String>>>,
    will: Arc<Mutex<Option<Arc<crate::wits::Will>>>>,
    prompt: Arc<Mutex<Box<dyn crate::prompt::PromptBuilder + Send + Sync>>>,
}

impl Clone for Voice {
    fn clone(&self) -> Self {
        Self {
            chatter: self.chatter.clone(),
            mouth: self.mouth.clone(),
            events: self.events.clone(),
            ready: AtomicBool::new(self.ready.load(Ordering::SeqCst)),
            extra_prompt: self.extra_prompt.clone(),
            will: self.will.clone(),
            prompt: self.prompt.clone(),
        }
    }
}

impl Voice {
    pub fn new(
        chatter: Arc<dyn Chatter>,
        mouth: Arc<dyn Mouth + Send + Sync>,
        events: broadcast::Sender<Event>,
    ) -> Self {
        Self {
            chatter,
            mouth: Arc::new(Mutex::new(mouth)),
            events,
            ready: AtomicBool::new(true),
            extra_prompt: Arc::new(Mutex::new(None)),
            will: Arc::new(Mutex::new(None)),
            prompt: Arc::new(Mutex::new(Box::new(crate::prompt::VoicePrompt)
                as Box<dyn crate::prompt::PromptBuilder + Send + Sync>)),
        }
    }

    pub fn set_mouth(&self, mouth: Arc<dyn Mouth + Send + Sync>) {
        *self.mouth.lock().unwrap() = mouth;
    }

    pub fn set_will(&self, will: Arc<crate::wits::Will>) {
        *self.will.lock().unwrap() = Some(will);
    }

    pub fn set_prompt<P>(&self, prompt: P)
    where
        P: crate::prompt::PromptBuilder + Send + Sync + 'static,
    {
        *self.prompt.lock().unwrap() = Box::new(prompt);
    }

    pub fn permit(&self, prompt: Option<String>) {
        if self.ready.swap(true, Ordering::SeqCst) {
            return;
        }
        *self.extra_prompt.lock().unwrap() = prompt;
    }

    /// Returns `true` if the voice is currently permitted to speak.
    pub fn ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }

    pub async fn update_prompt_context(&self, ctx: &str) {
        self.chatter.update_prompt_context(ctx).await;
    }

    pub async fn take_turn(&self, system_prompt: &str, history: &[Message]) -> anyhow::Result<()> {
        info!("voice take_turn called");
        if !self.ready.swap(false, Ordering::SeqCst) {
            info!("voice not ready, returning early");
            return Ok(());
        }
        info!("voice permitted, generating speech");
        let extra = self.extra_prompt.lock().unwrap().take();
        let base = { self.prompt.lock().unwrap().build(system_prompt) };
        let prompt = if let Some(extra) = extra {
            format!("{}\n{}", base, extra)
        } else {
            base
        };
        info!(%prompt, "voice prompt");
        if let Ok(mut stream) = self.chatter.chat(&prompt, history).await {
            let mut full = String::new();
            let mut segmenter = SentenceSegmenter::new();
            while let Some(chunk_res) = stream.next().await {
                match chunk_res {
                    Ok(chunk) => {
                        debug!("chunk received: {}", chunk);
                        if !chunk.trim().is_empty() {
                            let _ = self.events.send(Event::StreamChunk(chunk.clone()));
                        }
                        full.push_str(&chunk);
                        for sentence in segmenter.push_str(&chunk) {
                            self.emit_sentence(&sentence).await;
                        }
                    }
                    Err(e) => {
                        warn!(?e, "llm stream error");
                        break;
                    }
                }
            }
            for sentence in segmenter.finish() {
                self.emit_sentence(&sentence).await;
            }
            info!(%full, "voice full response");
            let will = { self.will.lock().unwrap().clone() };
            if let Some(w) = will {
                w.handle_llm_output(&full).await;
            } else {
                debug!("Will not set; skipping output handling");
            }
        }
        Ok(())
    }

    async fn emit_sentence(&self, sentence: &str) {
        let trimmed = sentence.trim();
        if trimmed.is_empty() {
            return;
        }
        info!("assistant speaking: {}", trimmed);
        let (text, emojis) = extract_emojis(trimmed);
        for e in emojis {
            let _ = self.events.send(Event::EmotionChanged(e.clone()));
        }
        if !text.trim().is_empty() {
            tokio::time::sleep(Duration::from_millis(5)).await;
            let mouth = { self.mouth.lock().unwrap().clone() };
            mouth.speak(trimmed).await;
        }
    }
}

pub fn extract_emojis(text: &str) -> (String, Vec<String>) {
    use unicode_segmentation::UnicodeSegmentation;
    let mut plain = String::new();
    let mut emos = Vec::new();
    for g in UnicodeSegmentation::graphemes(text, true) {
        if emojis::get(g).is_some() {
            emos.push(g.to_string());
        } else {
            plain.push_str(g);
        }
    }
    (plain.trim().to_string(), emos)
}

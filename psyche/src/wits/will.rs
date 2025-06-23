use crate::Decision;
use crate::instruction::{Instruction, parse_instructions};
use crate::motorcall::InstructionRegistry;
use crate::prompt::PromptBuilder;
use crate::topics::{Topic, TopicBus};
use crate::traits::Doer;
use crate::{Decision, Impression, Stimulus, WitReport};
use async_trait::async_trait;
use lingproc::Instruction as LlmInstruction;
use quick_xml::{Reader, events::Event};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tracing::{debug, info};

/// Decide Pete's next action or speech using a language model.
///
/// `Will` buffers summaries of recent instants or moments and on [`tick`] asks
/// an LLM what Pete should do next. Any `<instruction>` style tags are parsed
/// and published on [`Topic::Instruction`]. The full decision is returned as an
/// [`Impression<Decision>`].
pub struct Will {
    doer: Arc<dyn Doer>,
    prompt: crate::prompt::WillPrompt,
    tx: Option<broadcast::Sender<WitReport>>,
    motor_registry: InstructionRegistry,
    buffer: Mutex<Vec<Impression<String>>>,
    bus: TopicBus,
}

impl Will {
    /// Debug label used for debug reporting.
    pub const LABEL: &'static str = "Will";

    /// Create a new `Will` using `bus` and an LLM `doer`.
    pub fn new(bus: TopicBus, doer: Arc<dyn Doer>) -> Self {
        Self::with_debug(bus, doer, None)
    }

    /// Create a `Will` that optionally emits [`WitReport`]s using `tx`.
    pub fn with_debug(
        bus: TopicBus,
        doer: Arc<dyn Doer>,
        tx: Option<broadcast::Sender<WitReport>>,
    ) -> Self {
        Self {
            doer,
            prompt: crate::prompt::WillPrompt,
            tx,
            motor_registry: InstructionRegistry::default(),
            buffer: Mutex::new(Vec::new()),
            bus,
        }
    }

    /// Replace the prompt builder.
    pub fn set_prompt(&mut self, prompt: crate::prompt::WillPrompt) {
        self.prompt = prompt;
    }

    /// Get mutable access to the instruction registry.
    pub fn motor_registry_mut(&mut self) -> &mut InstructionRegistry {
        &mut self.motor_registry
    }

    /// Allow the given [`Voice`](crate::voice::Voice) to speak using an optional
    /// instruction override.
    pub fn command_voice_to_speak(&self, voice: &crate::voice::Voice, prompt: Option<String>) {
        voice.permit(prompt);
    }

    /// Parse and route XML-style motor invocations in `output`.
    pub async fn handle_llm_output(&self, output: &str) {
        let mut reader = Reader::from_str(output);
        reader.trim_text(true);
        let mut buf = Vec::new();
        let mut tag: Option<(String, HashMap<String, String>)> = None;
        let mut content = String::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    let mut attrs = HashMap::new();
                    for a in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(a.key.as_ref()).to_string();
                        let val = a.unescape_value().unwrap_or_default().to_string();
                        attrs.insert(key, val);
                    }
                    tag = Some((name, attrs));
                }
                Ok(Event::Text(t)) => {
                    if tag.is_some() {
                        content.push_str(&t.unescape().unwrap_or_default());
                    }
                }
                Ok(Event::End(_)) => {
                    if let Some((name, attrs)) = tag.take() {
                        self.motor_registry
                            .invoke(&name, attrs, content.clone())
                            .await;
                        content.clear();
                    }
                }
                Ok(Event::Eof) => break,
                _ => {}
            }
            buf.clear();
        }
    }
}

#[async_trait]
impl crate::wit::Wit for Will {
    type Input = Impression<String>;
    type Output = Decision;

    async fn observe(&self, input: Self::Input) {
        self.buffer.lock().unwrap().push(input);
    }

    async fn tick(&self) -> Vec<Impression<Self::Output>> {
        let inputs = {
            let mut buf = self.buffer.lock().unwrap();
            if buf.is_empty() {
                return Vec::new();
            }
            let data = buf.clone();
            buf.clear();
            data
        };
        let input = inputs
            .last()
            .and_then(|i| i.stimuli.last())
            .map(|s| s.what.clone())
            .unwrap_or_default();
        let llm_instruction = LlmInstruction {
            command: self.prompt.build(&input),
            images: Vec::new(),
        };
        info!(prompt = %llm_instruction.command, "will prompt");
        let resp = match self.doer.follow(llm_instruction.clone()).await {
            Ok(r) => r,
            Err(e) => {
                debug!(?e, "will doer failed");
                return Vec::new();
            }
        };
        info!(response = %resp, "will response");
        if let Some(tx) = &self.tx {
            if crate::debug::debug_enabled(Self::LABEL).await {
                let _ = tx.send(WitReport {
                    name: Self::LABEL.into(),
                    prompt: llm_instruction.command.clone(),
                    output: resp.trim().to_string(),
                });
            }
        }
        self.handle_llm_output(&resp).await;
        let instructions = parse_instructions(&resp);
        if instructions.is_empty() {
            return Vec::new();
        }
        for ins in &instructions {
            self.bus.publish(Topic::Instruction, ins.clone());
        }
        let decision = Decision {
            text: resp.trim().to_string(),
            instructions,
        };
        vec![Impression::new(
            vec![Stimulus::new(decision.clone())],
            decision.text.clone(),
            None::<String>,
        )]
    }

    fn debug_label(&self) -> &'static str {
        Self::LABEL
    }
}

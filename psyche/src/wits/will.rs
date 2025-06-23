use crate::motorcall::MotorRegistry;
use crate::prompt::PromptBuilder;
use crate::{
    Impression, Stimulus, Summarizer,
    ling::{Doer, Instruction},
};
use async_trait::async_trait;
use quick_xml::{Reader, events::Event};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::info;

/// Decide Pete's next action or speech using a language model.
///
/// The Will listens for [`Instant`] impressions from the Quick and
/// interprets them to produce behavioral tags. It does not emit new
/// impressions itself. Instead, it forwards tags like `<say>` to the
/// appropriate motors so Pete can respond immediately.
///
/// `Will` sends the given situation summary to a [`Doer`] with a brief
/// prompt asking for a single sentence describing what Pete should do
/// or say next. The decision is returned as an [`Impression`].
///
/// # Example
/// ```no_run
/// # use psyche::{Will, ling::{Doer, Instruction}, Impression, Stimulus, Summarizer};
/// # use async_trait::async_trait;
/// # struct Dummy;
/// # #[async_trait]
/// # impl Doer for Dummy {
/// #   async fn follow(&self, _i: Instruction) -> anyhow::Result<String> {
/// #       Ok("Speak.".to_string())
/// #   }
/// # }
/// # #[tokio::main]
/// # async fn main() {
/// let will = Will::new(Box::new(Dummy));
/// let imp = will
///     .digest(&[Impression::new(
///         vec![Stimulus::new("greet the user".to_string())],
///         "",
///         None::<String>,
///     )])
///     .await
///     .unwrap();
/// assert_eq!(imp.summary, "Speak.");
/// # }
/// ```
#[derive(Clone)]
pub struct Will {
    doer: Arc<dyn Doer>,
    prompt: crate::prompt::WillPrompt,
    tx: Option<broadcast::Sender<crate::WitReport>>,
    motor_registry: MotorRegistry,
}

impl Will {
    /// Debug label for this summarizer.
    pub const LABEL: &'static str = "Will";
    /// Create a new `Will` using the provided [`Doer`].
    pub fn new(doer: Box<dyn Doer>) -> Self {
        Self {
            doer: doer.into(),
            prompt: crate::prompt::WillPrompt,
            tx: None,
            motor_registry: MotorRegistry::default(),
        }
    }

    /// Create a `Will` that emits [`WitReport`]s using `tx`.
    pub fn with_debug(doer: Box<dyn Doer>, tx: broadcast::Sender<crate::WitReport>) -> Self {
        Self {
            doer: doer.into(),
            prompt: crate::prompt::WillPrompt,
            tx: Some(tx),
            motor_registry: MotorRegistry::default(),
        }
    }

    /// Replace the prompt builder.
    pub fn set_prompt(&mut self, prompt: crate::prompt::WillPrompt) {
        self.prompt = prompt;
    }

    /// Get mutable access to the motor registry.
    pub fn motor_registry_mut(&mut self) -> &mut MotorRegistry {
        &mut self.motor_registry
    }

    /// Allow the given [`Voice`] to speak using an optional instruction
    /// override.
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
impl Summarizer<String, String> for Will {
    async fn digest(&self, inputs: &[Impression<String>]) -> anyhow::Result<Impression<String>> {
        let input = inputs
            .last()
            .and_then(|i| i.stimuli.last())
            .map(|s| s.what.clone())
            .unwrap_or_default();
        let instruction = Instruction {
            command: self.prompt.build(&input),
            images: Vec::new(),
        };
        info!(prompt = %instruction.command, "will prompt");
        let resp = self.doer.follow(instruction.clone()).await?;
        info!(response = %resp, "will response");
        let decision = resp.trim().to_string();
        if let Some(tx) = &self.tx {
            if crate::debug::debug_enabled(Self::LABEL).await {
                let _ = tx.send(crate::WitReport {
                    name: Self::LABEL.into(),
                    prompt: instruction.command.clone(),
                    output: decision.clone(),
                });
            }
        }
        Ok(Impression::new(
            vec![Stimulus::new(decision.clone())],
            decision,
            None::<String>,
        ))
    }
}

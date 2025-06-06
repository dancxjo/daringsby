use crate::{fond::FondDuCoeur, witness::WitnessAgent, genie::Genie};
use voice::{ThinkMessage, VoiceAgent};

pub struct Psyche {
    pub here_and_now: String,
    fond: FondDuCoeur,
}

impl Psyche {
    pub fn new() -> Self {
        Self { here_and_now: String::new(), fond: FondDuCoeur::new() }
    }

    pub async fn tick<V: VoiceAgent>(&mut self, witness: &WitnessAgent, voice: &V) -> ThinkMessage {
        if let Some(s) = witness.last().cloned() {
            let _ = self.fond.feel(s).await;
            if let Ok(sum) = self.fond.consult().await {
                self.here_and_now = sum;
            }
        }
        let content = voice.narrate(&self.here_and_now).await;
        ThinkMessage { content }
    }

    pub fn fond_identity(&self) -> &str {
        self.fond.identity()
    }
}

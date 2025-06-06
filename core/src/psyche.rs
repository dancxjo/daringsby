use voice::{ThinkMessage, VoiceAgent};

use crate::witness::WitnessAgent;

pub async fn tick<V: VoiceAgent>(witness: &WitnessAgent, voice: &V) -> ThinkMessage {
    let context = witness.last_text().unwrap_or("");
    let content = voice.narrate(context).await;
    ThinkMessage { content }
}

pub mod witness;
pub mod psyche;

pub fn placeholder() {
    println!("core module initialized");
}

#[cfg(test)]
mod tests {
    use super::*;
    use sensor::Sensation;
    use voice::{ThinkMessage, VoiceAgent};

    struct MockNarrator;

    #[async_trait::async_trait]
    impl VoiceAgent for MockNarrator {
        async fn narrate(&self, context: &str) -> String {
            format!("echo: {context}")
        }
    }

    #[tokio::test]
    async fn bridge_sensation_to_think() {
        let mut witness = witness::WitnessAgent::default();
        witness.ingest(Sensation { text: "hello".into() });
        let narrator = MockNarrator;
        let msg = psyche::tick(&witness, &narrator).await;
        assert_eq!(msg, ThinkMessage { content: "echo: hello".into() });
    }
}

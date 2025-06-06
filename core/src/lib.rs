#![doc(test(no_crate_inject))]

pub mod witness;
pub mod psyche;
pub mod genie;
pub mod fond;

pub fn placeholder() {
    println!("core module initialized");
}

#[cfg(test)]
mod tests {
    use super::*;
    use sensor::Sensation;
    use voice::{ThinkMessage, VoiceAgent};
    use crate::genie::{Genie, GenieError};

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
        witness.ingest(Sensation::new("hello", None::<String>));
        let narrator = MockNarrator;
        let mut psyche = psyche::Psyche::new();
        let msg = psyche.tick(&witness, &narrator).await;
        assert_eq!(psyche.here_and_now, "hello");
        assert_eq!(msg, ThinkMessage { content: "echo: hello".into() });
    }

    struct FixedGenie {
        summary: String,
        felt: usize,
    }

    #[async_trait::async_trait]
    impl Genie for FixedGenie {
        async fn feel(&mut self, _s: Sensation) { self.felt += 1; }
        async fn consult(&mut self) -> Result<String, GenieError> { Ok(self.summary.clone()) }
    }

    #[tokio::test]
    async fn fixed_genie_works() {
        let mut g = FixedGenie { summary: "ok".into(), felt: 0 };
        g.feel(Sensation::new("hi", None::<String>)).await;
        assert_eq!(g.felt, 1);
        let out = g.consult().await.unwrap();
        assert_eq!(out, "ok");
    }

    #[tokio::test]
    async fn fond_updates_identity() {
        let mut fond = fond::FondDuCoeur::new();
        fond.feel(Sensation::new("I saw a bird", None::<String>)).await;
        fond.feel(Sensation::new("It was blue", None::<String>)).await;
        let summary = fond.consult().await.unwrap();
        assert!(summary.contains("I saw a bird"));
        assert!(summary.contains("It was blue"));
    }
}

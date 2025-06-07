//! Core abstractions for Pete Daringsby's mind.
//!
//! This crate wires together the [`WitnessAgent`] which perceives incoming
//! [`Sensation`](sensor::Sensation)s with the [`Psyche`] that summarizes them
//! using a [`Genie`] implementation such as [`FondDuCoeur`].
//!
//! ```
//! use core::{psyche::Psyche, witness::WitnessAgent};
//! use sensor::Sensation;
//!
//! let mut witness = WitnessAgent::default();
//! witness.ingest(Sensation::new("a passing thought", None::<String>));
//! struct SilentVoice;
//! #[async_trait::async_trait]
//! impl voice::VoiceAgent for SilentVoice {
//!     async fn narrate(&self, _c: &str) -> voice::VoiceOutput {
//!         voice::VoiceOutput {
//!             think: voice::ThinkMessage { content: String::new() },
//!             say: None,
//!             emote: None,
//!         }
//!     }
//! }
//! let mut psyche = Psyche::new(witness, SilentVoice);
//! // a real VoiceAgent would narrate this context
//! let context = psyche.here_and_now.clone();
//! assert!(context.is_empty());
//! ```
#![doc(test(no_crate_inject))]

pub mod fond;
pub mod genie;
pub mod psyche;
pub mod witness;
pub mod prompt_builder;
pub mod ethics;

/// Emit a simple initialization message.
///
/// This is mostly here so each crate exposes at least one public function.
pub fn placeholder() {
    println!("core module initialized");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genie::{Genie, GenieError};
    use memory::{Experience, Memory};
    use sensor::Sensation;
    use std::pin::Pin;
    use voice::{EmoteMessage, SayMessage, ThinkMessage, VoiceAgent, VoiceOutput};

    struct MockNarrator;

    #[async_trait::async_trait]
    impl VoiceAgent for MockNarrator {
        async fn narrate(&self, context: &str) -> VoiceOutput {
            VoiceOutput {
                think: ThinkMessage { content: format!("echo: {context}") },
                say: Some(SayMessage { content: context.into() }),
                emote: Some(EmoteMessage { emoji: "😊".into() }),
            }
        }
    }

    #[tokio::test]
    async fn bridge_sensation_to_think() {
        let mut witness = witness::WitnessAgent::default();
        witness.ingest(Sensation::new("hello", None::<String>));
        let narrator = MockNarrator;
        let mut psyche = psyche::Psyche::new(witness, narrator);
        let msg = psyche.tick().await;
        assert_eq!(psyche.here_and_now, "hello");
        assert_eq!(msg.think.content, "echo: hello");
        assert_eq!(msg.say, Some(SayMessage { content: "hello".into() }));
        assert_eq!(msg.emote, Some(EmoteMessage { emoji: "😊".into() }));
    }

    struct FixedGenie {
        summary: String,
        felt: usize,
    }

    #[async_trait::async_trait]
    impl Genie for FixedGenie {
        async fn feel(&mut self, _s: Sensation) {
            self.felt += 1;
        }
        async fn consult(&mut self) -> Result<String, GenieError> {
            Ok(self.summary.clone())
        }
    }

    #[tokio::test]
    async fn fixed_genie_works() {
        let mut g = FixedGenie {
            summary: "ok".into(),
            felt: 0,
        };
        g.feel(Sensation::new("hi", None::<String>)).await;
        assert_eq!(g.felt, 1);
        let out = g.consult().await.unwrap();
        assert_eq!(out, "ok");
    }

    #[tokio::test]
    async fn fond_updates_identity() {
        let mut fond = fond::FondDuCoeur::new();
        fond.feel(Sensation::new("I saw a bird", None::<String>))
            .await;
        fond.feel(Sensation::new("It was blue", None::<String>))
            .await;
        let summary = fond.consult().await.unwrap();
        assert!(summary.contains("I saw a bird"));
        assert!(summary.contains("It was blue"));
    }

    struct InMem {
        inner: std::sync::Mutex<Vec<Experience>>,
    }

    #[async_trait::async_trait]
    impl Memory for InMem {
        async fn store(&self, exp: Experience) -> Result<(), memory::MemoryError> {
            self.inner.lock().unwrap().push(exp);
            Ok(())
        }
    }

    struct StubLLM;

    #[async_trait::async_trait]
    impl llm::traits::LLMClient for StubLLM {
        async fn stream_chat(
            &self,
            _model: &str,
            _prompt: &str,
        ) -> Result<
            Pin<
                Box<
                    dyn futures_util::stream::Stream<Item = Result<String, llm::traits::LLMError>>
                        + Send,
                >,
            >,
            llm::traits::LLMError,
        > {
            use futures_util::stream;
            Ok(Box::pin(stream::iter(vec![
                Ok("summary one. ".to_string()),
                Ok("summary two".to_string()),
            ])))
        }

        async fn embed(
            &self,
            _model: &str,
            _input: &str,
        ) -> Result<Vec<f32>, llm::traits::LLMError> {
            Ok(vec![0.0])
        }
    }

    #[tokio::test]
    async fn witness_feel_and_store() {
        let mut witness = witness::WitnessAgent::default();
        let llm = StubLLM;
        let mem = InMem {
            inner: std::sync::Mutex::new(Vec::new()),
        };
        let exp = witness
            .feel(Sensation::new("beat", None::<String>), &llm)
            .await
            .unwrap();
        assert_eq!(exp.explanation, "summary one. ");
        witness.witness(exp, &mem).await.unwrap();
        assert_eq!(mem.inner.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn witness_summarizes_here_and_now() {
        let mut witness = witness::WitnessAgent::default();
        witness.ingest(Sensation::new("first", None::<String>));
        witness.ingest(Sensation::new("second", None::<String>));
        let mut fond = fond::FondDuCoeur::new();
        let sum = witness.summarize(&mut fond).await.unwrap();
        assert_eq!(sum, "first second");
    }
}

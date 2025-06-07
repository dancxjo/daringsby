# Package Overview

This document lists each crate in the Pete Daringsby workspace with a short description and example usage.

- **core** – core abstractions connecting sensors, memory and voice. Includes the `PromptBuilder` for constructing LLM prompts with customizable reflection formats.
  ```rust
  use core::{psyche::Psyche, witness::WitnessAgent};
  use sensor::Sensation;
  use voice::{VoiceOutput, ThinkMessage, VoiceAgent};

  struct DummyVoice;
  #[async_trait::async_trait]
  impl VoiceAgent for DummyVoice {
      async fn narrate(&self, _c: &str) -> VoiceOutput {
          VoiceOutput {
              think: ThinkMessage { content: String::new() },
              say: None,
              emote: None,
          }
      }
  }

  let mut wit = WitnessAgent::default();
  wit.ingest(Sensation::new("hi", None::<String>));
  let psyche = Psyche::new(wit, DummyVoice);
  ```
- **memory** – store [`Experience`](../memory/src/experience.rs) objects in Qdrant and Neo4j.
- **voice** – manage LLM conversations and produce responses.
- **sensor** – emit [`Sensation`](../sensor/src/sensation.rs) values from various inputs.
- **tts** – turn text into audio using Coqui TTS.
- **llm** – language model client and routing utilities.
- **net** – networking helpers.
- **vision** – stubs for future computer vision work.
- **sensation-server** – axum based WebSocket server exposing `/ws` and `/devpanel`.
- **sensation-tester** – CLI utility to send mock sensor events.

# Package Overview

This document lists each crate in the Pete Daringsby workspace with a short description and example usage.

- **core** – core abstractions connecting sensors, memory and voice.
  ```rust
  use core::{psyche::Psyche, witness::WitnessAgent};
  use sensor::Sensation;
  let mut wit = WitnessAgent::default();
  wit.ingest(Sensation::new("hi", None::<String>));
  let psyche = Psyche::new();
  ```
- **memory** – store [`Experience`](../memory/src/experience.rs) objects in Qdrant and Neo4j.
- **voice** – manage LLM conversations and produce responses.
- **sensor** – emit [`Sensation`](../sensor/src/sensation.rs) values from various inputs.
- **tts** – turn text into audio using Coqui TTS.
- **llm** – language model client and routing utilities.
- **net** – networking helpers.
- **vision** – stubs for future computer vision work.

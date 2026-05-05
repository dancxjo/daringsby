# Daringsby Workspace

This repository implements Pete Daringsby: a Rust-based artificial agent with real-time perception, emotional awareness, and expressive behavior. It is organized as a workspace with three crates:

* **`psyche`** – core cognition (the mind of Pete)
* **`lingproc`** – LLM and embedding abstraction layer
* **`pete`** – the host binary, tying sensors and outputs to Pete’s cognitive loop

---

## 🧠 Architecture Overview

Pete's cognitive engine is structured as a sequence of `Wit` modules. Each Wit ingests lower-level impressions and emits higher-level `Impression<T>` thoughts. These are stored as `Experience<T>`s with vector embeddings.

Key concepts:

* `Sensation`: Raw input from sensors
* `Stimulus<T>`: Timestamped observation of input or a prior impression
* `Impression<T>`: Interpretation of one or more stimuli with summary text and optional emoji
* `Experience<T>`: Remembered impression with vector embedding and ID

### Primary Wits

* **Quick**: Groups `Sensation`s into an immediate `Impression`
* **Combobulator**: Generates a concise `Impression` of what just happened
* **Memory**: Stores impressions in Neo4j and Qdrant
* **Heart**: Derives emotional state (emoji)
* **Will**: Issues behavioral instructions
* **Voice**: Generates responses when permitted

---

## 💻 Example Usage

```rust
use lingproc::OllamaProvider;
use psyche::Psyche;

let narrator = OllamaProvider::new("http://localhost:11434", "gemma3").unwrap();
let voice = OllamaProvider::new("http://localhost:11434", "gemma3").unwrap();
let vectorizer = OllamaProvider::new("http://localhost:11434", "embeddinggemma").unwrap();

use psyche::{Ear, Mouth};
use async_trait::async_trait;

// Stub implementations for demonstration
struct DummyMouth;
#[async_trait] impl Mouth for DummyMouth { /* ... */ }

struct DummyEar;
#[async_trait] impl Ear for DummyEar { /* ... */ }

let psyche = Psyche::new(
    Box::new(narrator),
    Box::new(voice),
    Box::new(vectorizer),
    std::sync::Arc::new(psyche::NoopMemory),
    std::sync::Arc::new(DummyMouth),
    std::sync::Arc::new(DummyEar),
);
psyche.set_system_prompt("Respond in two sentences max.");
psyche.set_speak_when_spoken_to(true);
psyche.run().await;
```

---

## 🚀 Running the System

The easiest way to work with Pete is through [`just`](https://github.com/casey/just).
The repo `justfile` loads `.env` automatically before running commands.

```sh
just run
```

Pete uses separate Ollama models for text generation and embeddings. Pull both
before running with the default configuration:

```sh
ollama pull gemma3
ollama pull embeddinggemma
```

For debug logs:

```sh
just debug
```

Extra CLI args are forwarded to the `pete` binary:

```sh
just run --addr 127.0.0.1:3000
```

Without `just`, use:

```sh
cargo run -p pete --bin pete
```

### ASR Model

Server-side ASR is enabled by default when a Whisper model is available. Fetch
the default `base.en` model and voice embedding model with:

```sh
just fetch
```

That writes `models/whisper/ggml-base.en.bin` and
`models/voice/speaker_embedding_extractor.onnx`, which Pete discovers
automatically. To fetch a different Whisper model:

```sh
just fetch tiny.en
just fetch small.en
```

You can also set `WHISPER_MODEL` or `VOICE_EMBEDDING_MODEL` in `.env` to point
at custom model paths.

---

## 🌐 Web Interface

Visit [`http://localhost:3000/`](http://localhost:3000/) after launch.

* WebSocket connection at `/ws`
* Debug info now included on `/ws` as `Think` events
* JSON endpoints:

  * `/conversation` – full log
  * `/debug/psyche` – tick stats

Events from Pete include speech, emotion changes, wit reports and conversation updates:

```json
{ "type": "say", "data": { "words": "hi", "audio": "..." } }
{ "type": "Emote", "data": "😊" }
```

---

## 🧪 Testing & Simulation

Run tests:

```sh
just test
```

Rust only:

```sh
just test-rust
```

Frontend only:

```sh
just test-frontend
```

Simulate input:

```sh
just simulate-text "hello"
just simulate-image some.png
```

---

## 🔧 Build Notes

* List commands: `just`
* Format: `just fmt`
* Check formatting: `just fmt-check`
* Check Rust: `just check`
* Lint: `just clippy`
* Logging: `RUST_LOG=debug`
* Features: `tts`, `asr`, `geo`, `eye`, `face`, `all-sensors`

---

## 📎 Related Modules

* `lingproc::segment_text_into_sentences()` – splits text:

```rust
let s = "Hello. World.";
let parts = lingproc::segment_text_into_sentences(s);
assert_eq!(parts, vec!["Hello.", "World."]);
```

---

## 🧠 Project Goals

* Maintain realism: Pete only speaks about what he perceives, remembers, or is told
* Modular cognition: composable Wits allow reuse and experimentation
* Agent narration: internal thoughts form a coherent and evolving identity

---

For deeper internals, see [`docs/Agent Overview`](./agents.md) or explore the `psyche::wits` module.

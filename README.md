# Daringsby Workspace

This repository implements Pete Daringsby: a Rust-based artificial agent with real-time perception, emotional awareness, and expressive behavior. It is organized as a workspace with three crates:

* **`psyche`** – core cognition (the mind of Pete)
* **`lingproc`** – LLM and embedding abstraction layer
* **`pete`** – the host binary, tying sensors and outputs to Pete’s cognitive loop

---

## 🧠 Architecture Overview

Pete's cognitive engine is structured as a sequence of `Wit` modules. Each Wit ingests lower-level impressions and emits higher-level `Impression<T>` thoughts. These are stored as `Experience<T>`s with vector embeddings.

Key concepts:

* `Sensation`: raw input plus `occurred_at`, a quick first-person present-tense `how` sentence, and optional `how_formed_at`
* `Stimulus<T>`: timestamped observation of input or a prior impression
* `Impression<T>`: interpretation of one or more stimuli with summary text and optional emoji
* `Experience<T>`: remembered impression with vector embedding and ID

Graph storage keeps vectors as fields on the nodes they describe, rather than
requiring separate vector nodes for ordinary sensation and impression records.

### Primary Wits

* **Quick**: Groups `Sensation`s into an immediate `Impression`
* **Combobulator**: Generates a concise `Impression` of what just happened
* **Memory**: Stores impressions in Neo4j and Qdrant
* **Heart**: Derives emotional state (emoji)
* **Will**: Issues behavioral instructions
* **Voice**: Generates responses when permitted

### Face Expression Control

Pete's face expression is intentionally driven through two overlapping paths:

* **Combobulation mirroring**: the `face` binary polls the latest
  combobulation emoji and sends it to the browser as an `Emote` WebSocket
  payload. This preserves a low-latency emotional readout of the current
  awareness.
* **Will expression**: the standalone `will` binary polls the latest
  combobulation text, prompts with Pete's system prompt without a timeline, and
  asks for a chat-formatted response including `<thought>` tags and an emoji. It stores the active
  decision as impression sensations: `I think: ...`, `I ought to say: ...`, and `I turn my face into a $EMOJI.`
* **Face proprioception**: whenever the `face` binary actually emits an emoji
  to the browser, it also stores a redundant impression sensation:
  `I feel my face turn into a $EMOJI.`

This scheme gives Pete a "pseudoconscious" control channel over the face:
`Will` can intentionally choose an outward expression based on the latest
awareness summary, and that chosen expression becomes graph-visible as an act.
At the same time, combobulation mirroring remains active as a separate,
ambient path. Those direct combobulation updates behave like microexpressions:
small, fast facial shifts that surface from current awareness before or between
explicit Will decisions. The redundant face sensation closes the loop by
recording that the presented face changed, so later cognition can treat the
expression as both an intended action and a felt bodily state.

---

## 💻 Example Usage

```rust
use lingproc::OllamaProvider;
use psyche::Psyche;

let narrator = OllamaProvider::new("http://localhost:11434", "gpt-oss").unwrap();
let voice = OllamaProvider::new("http://localhost:11434", "gpt-oss").unwrap();
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

To build Docker images and stopped Compose containers for the same component
binaries that `just run` starts:

```sh
just build
```

Pass one component name to leave it out so you can run that binary locally:

```sh
just build will
```

The Pete component services live behind the Compose `pete` profile. Existing
infrastructure services such as `nginx`, `qdrant`, `neo4j`, and `tts` still work
without that profile.

The vector cluster loop is not part of the default run set. Start it explicitly
when you want a clustering pass or background clustering:

```sh
just cluster --once
just cluster
```

Start the created component containers with:

```sh
docker compose --profile pete start
```

For Docker, the cluster service is behind a separate profile:

```sh
docker compose --profile cluster up cluster
```

Pete uses separate Ollama models for text generation, vision, and embeddings.
Pull all three before running with the default configuration:

```sh
ollama pull gpt-oss
ollama pull gemma4
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

Server-side ASR is enabled by default when a Whisper model is available. The
default Pete build compiles Whisper with CUDA support and enables GPU loading
unless `ASR_USE_GPU=false` is set. Fetch the default fast `small.en` model and
the voice embedding model with:

```sh
just fetch
```

That writes `models/whisper/ggml-small.en.bin` and
`models/voice/speaker_embedding_extractor.onnx`, which Pete discovers
automatically. The transcription loop prefers `small.en`; to fetch a different
Whisper model:

```sh
just fetch tiny.en
just fetch base.en
just fetch small.en
```

You can also set `WHISPER_MODEL` or `VOICE_EMBEDDING_MODEL` in `.env` to point
at custom model paths for the live ASR service.

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

For local HTTPS without managing certificates by hand, start the nginx TLS
wrapper from Docker Compose:

```sh
docker compose up nginx
```

It generates and persists a self-signed localhost certificate, then proxies:

* [`https://localhost:3443/`](https://localhost:3443/) -> `http://127.0.0.1:3000/`
* [`https://localhost:3444/`](https://localhost:3444/) -> `http://127.0.0.1:3001/`

Override `PETE_TLS_PORT_3000`, `PETE_TLS_PORT_3001`, `PETE_UPSTREAM_3000`, or
`PETE_UPSTREAM_3001` in `.env` if you want different ports.

### Psychic Graph Client

Run the graph browser beside the face capture server:

```sh
cargo run -p pete --bin psychic -- --addr 127.0.0.1:3001
```

Visit [`http://localhost:3001/`](http://localhost:3001/). Psychic streams the
latest Neo4j graph window over `/ws` and exposes the same snapshot at `/graph`.
Set `PSYCHIC_GRAPH_LIMIT` or pass `--graph-limit` to adjust the snapshot size.

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

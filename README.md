# Pete Daringsby - Rust Workspace

Pete Daringsby is a narrative-first artificial consciousness implemented as a set
of cooperating Rust crates. The project explores how modular "subagents" can
combine sensor input, memory and language models into a continuous first-person
story. Each crate in the workspace represents one piece of that puzzle.

Detailed API notes for each package live in
[docs/package_overview.md](docs/package_overview.md). See
[docs/architecture.md](docs/architecture.md) for a high-level overview of the
agent design. The summary below gives a quick sense of the layout.

## Workspace crates

- **core** – orchestrates subagents like `Witness` and `Voice` and exposes
  the `Psyche` type that binds them together.
- **net** – helpers for WebSocket messaging and client/server communication.
- **memory** – abstractions for storing [`Experience`](memory/src/experience.rs)
  objects in Qdrant and Neo4j.
- **voice** – language model coordination and conversation state management.
- **llm** – generic "language processor" utilities and the `LinguisticScheduler` for capability-based model selection.
- **tts** – converts LLM output into audio via Coqui TTS.
- **sensor** – audio, geolocation and filesystem listeners that emit
  [`Sensation`](sensor/src/sensation.rs) values.
- **vision** – webcam and face recognition helpers (currently stubbed).
- **sensation-server** – WebSocket backend with a small development panel.
- **sensation-tester** – CLI tool for sending mock sensor input during dev.

Run `cargo check` in the repository root to verify that all crates compile.
CI on GitHub automatically runs `cargo check` and `cargo test` for pushes and pull requests.

## Setup

1. Install Rust (stable) and Docker.
2. Copy `.env.example` to `.env` and set the environment variables described
   below.
3. Start the Coqui TTS server with `docker-compose up -d tts`.
4. Optional: run Whisper locally for ASR and configure its address in `.env`.
5. Run `docker-compose up -d qdrant neo4j` if you want the memory backends.

### Environment variables

| Name | Purpose |
| --- | --- |
| `OLLAMA_URL` | Base URL of the primary Ollama server |
| `OLLAMA_URLS` | Comma separated list of Ollama hosts for load balancing |
| `OLLAMA_MODEL` | LLM model identifier |
| `COQUI_URL` | Base URL of the Coqui TTS server |
| `SPEAKER` | Coqui speaker name |
| `QDRANT_URL` | Address of the Qdrant vector database |
| `NEO4J_URI` | Bolt URI for Neo4j |
| `NEO4J_USER` | Database username |
| `NEO4J_PASS` | Database password |

## Running

Start the WebSocket backend:

```bash
cargo run -p sensation-server
```

Use `sensation-tester` to send mock sensor input:

```bash
cargo run -p sensation-tester -- --help
```

## Testing

Run the full test suite with:

```bash
cargo test
```

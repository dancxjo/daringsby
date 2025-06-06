# Pete Daringsby - Rust Workspace

This repository contains the initial Rust workspace layout for the Pete Daringsby project. Each crate represents a subsystem used to build an async, actor-based assistant.

## Workspace crates

- **core** – fundamental types and orchestrators
- **net** – WebSocket and message protocol handling
- **memory** – abstractions for persistence layers like Neo4j and Qdrant
- **voice** – voice interface and language model coordination
- **tts** – text-to-speech integration
- **sensor** – audio, geolocation and filesystem listeners
- **vision** – webcam and face recognition helpers
- **sensation-server** – WebSocket backend with a simple dev panel
- **sensation-tester** – CLI tool for sending mock sensor input

Run `cargo check` in the repository root to verify that all crates compile.
CI on GitHub automatically runs `cargo check` and `cargo test` for pushes and pull requests.

## Setup

1. Install Rust (stable) and Docker.
2. Copy `.env.example` to `.env` and set `OLLAMA_URL`, `OLLAMA_MODEL`, `COQUI_URL` and `SPEAKER`.
3. Start the Coqui TTS server with `docker-compose up -d tts`.
4. Optional: run Whisper locally for ASR and configure its address in `.env`.

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

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

Run `cargo check` in the repository root to verify that all crates compile.
CI on GitHub automatically runs `cargo check` and `cargo test` for pushes and pull requests.

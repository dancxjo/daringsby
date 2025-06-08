# Daringsby

This workspace contains several Rust crates forming a small experimental AI stack.

- **lingproc** – interfaces with language models and provides processors for chat
  completion, embeddings and instruction following.
- **modeldb** – stores metadata about available AI models.
- **psyche** – basic types representing sensations, experiences and sensors.
- **memory** – abstractions and utilities for persisting information.

Tests and formatting can be run for the entire workspace:

```bash
cargo fmt --all
cargo test --all
```

The provided `docker-compose.yml` launches optional services such as text to
speech (Coqui TTS), Qdrant and Neo4j which are useful during development but not
required to run the unit tests.

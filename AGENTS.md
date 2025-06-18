# Agent Instructions

This repository is a Rust workspace.

## Setup

* Install the stable Rust toolchain.
* Ensure the `rustfmt` and `clippy` components are installed.
* Run `cargo fetch` to warm the cache before testing.

## Running & Testing

* Run tests with `cargo test` from the repository root.
* Format with `cargo fmt`.
* Use `tracing` macros for all logging.
* Initialize logging in binaries with `tracing_subscriber::fmt::init()`.

## Project Layout

* Crate `pete` depends on local crate `psyche`.
* Crates should be logically modular; split files beyond \~200 lines.

## Code Practices

* Prefer traits for abstraction (`Mouth`, `Ear`, `Countenance`, `Wit`).
* Use `Summarizer` when batching impressions into higher-level summaries.
* Document new traits with examples and unit tests.
* Prefer `AndMouth` when composing multiple `Mouth` implementations.
* Use `TrimMouth` to skip speaking empty/whitespace-only text.
* Use `EmojiMouth` to route emoji to the countenance instead of speaking them.
* Do **not** emit `Event::IntentionToSay` for empty or whitespace-only text.
* Skip sending `Event::StreamChunk` when the chunk is empty or whitespace.
* Build prompts using dedicated structs like `WillPrompt` and `HeartPrompt`.
* `ChannelMouth` emits `Event::IntentionToSay` per parsed sentence.
* `ChannelCountenance` emits `Event::EmotionChanged` on updates.
* `Conversation::add_*` merges consecutive same-role messages.

## Frontend

* Keep `index.html` minimal.
* It should connect to `ws://localhost:3000/ws`.
* Show WebSocket connection status for debugging.
* Use Alpine.js for client binding.
* Render chat log as `<ul>` with `<li>` per message.
* Style user and system messages distinctly for clarity.
* Keep `index.html` and `pete/build.rs` in sync.
* Front-end tests live under `frontend/` and run with `npm test`.
* Run `npm install` first if dependencies are missing.

## Communication

* Expose WebSocket chat at `/ws`, forwarding all `Psyche` events.
* SSE endpoints like `/chat` are deprecated; use WebSocket only.

## Audio / TTS

* The `tts` feature streams audio from Coqui TTS.
* Configure with `--tts-url` CLI flag.
* Build the `pete` binary with `--features tts` to enable audio.
* Stub TTS in tests to avoid delays.
* Do not include the `style_wav` parameter when calling Coqui TTS.

## Specialized Notes

* `Wit` runs asynchronously and infrequently — do not block main loop. Implement
  tick-based summarization when possible.
* Voice should **only** generate dialogue; all decisions routed through `Will`.
* Memory graph (Neo4j) and embedding DB (Qdrant) must stay in sync.
* Long-lived impressions are stored as `Impression<T>` with headline, detail, and raw data.

## Contributor Notes

* Use meaningful commit messages.
* Keep README examples up to date with public APIs.
* Document all new CLI arguments and environment flags.
* Avoid `echo $?`; rely on return values/output checks.

## LLM Integration

* Fast LLMs (e.g. Ollama) for `Will`, `Voice`, `Heart`, `Combobulator`.
* Slow/idle LLMs for `Memory`, `Narrator`.
* Only `Will` may invoke `Voice::take_turn`.
* `Countenance` trait should reflect current emotion via emoji.

## Additional Suggestions

* Consider adding unit tests that simulate full conversation loops (with mocked `Mouth`, `Ear`, `Voice`).
* Consider adding CLI test scaffolding for mocking TTS/Neo4j/Qdrant.
* Ensure that `Wit<Instant>` is fed only when it has sufficient `Sensation` inputs — fail early otherwise.
* Be mindful of the single-CPU assumption — prefer concurrency without heavy parallelism.
* When skipping speech for empty responses, increment the turn counter so the conversation loop can exit.
* Log Coqui TTS request URLs with `info!(%url, "requesting TTS")` to ease debugging misconfigured endpoints.

This document reflects the current cognitive and runtime architecture of Pete Daringsby. Keep it consistent with the latest design discussions and behavior changes.

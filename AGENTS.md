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

* Prefer traits for abstraction (`Mouth`, `Ear`, `Wit`).
* Use `Summarizer` when batching impressions into higher-level summaries.
* Document new traits with examples and unit tests.
* Sensors expose `description()` for prompt inclusion.
* Prefer `AndMouth` when composing multiple `Mouth` implementations.
* Use `TrimMouth` to skip speaking empty/whitespace-only text.
* Inline emoji in responses convey emotional tone.
* Do **not** emit `Event::IntentionToSay` for empty or whitespace-only text.
* Skip sending `Event::StreamChunk` when the chunk is empty or whitespace.
* Build prompts using dedicated structs like `WillPrompt`.
* `ChannelMouth` emits `Event::Speech` per parsed sentence without audio.
* `Conversation::add_*` merges consecutive same-role messages, inserting a space and trimming.
* Only the `Psyche` loop should append to `Conversation`; `Ear` implementations forward sensations without modifying the log.
* Use the `Motor` trait for host actions. Implementations live in `pete`.

## Frontend

Static assets live under `frontend/dist` and are served by the `pete` binary.
Navigate to `http://localhost:3000/` to load the web face which connects to
`ws://localhost:3000/ws`.

### Frontend Notes

The previous Deno-based client has been removed. Update the files in
`frontend/dist` directly to change the interface.
* Queue audio playback on the client so clips never overlap.
* Define CSS variables in `styles.css` to control colors and fonts.
* Reuse a single `<audio>` element for speech playback so controls remain visible.
* After playing speech audio, send an `Echo` message with the spoken text so the conversation log records assistant dialogue.
* Define CSS variables in `styles.css` to control colors and fonts.
* Keep the thought bubble hidden until there is text to display.
* Serve over HTTPS by passing `--tls-cert` and `--tls-key` to the `pete` binary.
* Canvas elements that repeatedly call `getImageData` must obtain their context
  with `{ willReadFrequently: true }`.

## Communication

* Expose WebSocket chat at `/ws`, forwarding all `Psyche` events.
* Debug information from Wits streams via `/debug`.
* The `EventBus` retains the last `WitReport` so new `/debug` subscribers see
  recent output immediately.
* SSE endpoints like `/chat` are deprecated; use WebSocket only.
* Text messages no longer trigger `Heard` responses.

## Audio / TTS

* The `tts` feature streams audio from Coqui TTS.
* Configure with `--tts-url` CLI flag.
* Build the `pete` binary with `--features tts` to enable audio.
* Stub TTS in tests to avoid delays.
* Do not include the `style_wav` parameter when calling Coqui TTS.
* Speech is emitted via `Event::Speech { text, audio }`.

## Specialized Notes

* `Wit` runs asynchronously and infrequently — do not block main loop. Implement
  tick-based summarization when possible.
* Voice should **only** generate dialogue; all decisions routed through `Will`.
* Memory graph (Neo4j) and embedding DB (Qdrant) must stay in sync.
* Long-lived memories are stored as `Experience<T>` combining an `Impression<T>` with an embedding and id.
* Use `Prehension` when buffering impressions for summarization. `Wit` is generic over input and output types.
* When adding new Wits that emit `WitReport`s, prefer constructors like
  `with_debug` and register them with `psyche.wit_sender()` so debug output is
  available during tests.
* Tests expecting a `WitReport` must enable the matching debug label with
  `psyche::debug::enable_debug(label).await`.

## Contributor Notes

* Use meaningful commit messages.
* Keep README examples up to date with public APIs.
* Document all new CLI arguments and environment flags.
* Avoid `echo $?`; rely on return values/output checks.
* Favor TDD/BDD when adding features; write failing tests first.
* Provide stub implementations for external ML components so tests run offline.
* There is no bundled frontend. Connect your own WebSocket client to
  `ws://localhost:3000/ws`.

## LLM Integration

* Fast LLMs (e.g. Ollama) for `Will`, `Voice`, `Combobulator`.
* Slow/idle LLMs for `Memory`, `Narrator`.
* Only `Will` may invoke `Voice::take_turn`.
* `Voice::take_turn` extracts emoji and emits `Event::EmotionChanged`.
* `Voice` will not speak until `Will::command_voice_to_speak` grants permission.
* `Voice::permit` is idempotent and returns early when already ready.
* `WillWit::tick` may call `voice.permit(Some(prompt))` to trigger speech when rules allow.

## Additional Suggestions

* Consider adding unit tests that simulate full conversation loops (with mocked `Mouth`, `Ear`, `Voice`).
* Consider adding CLI test scaffolding for mocking TTS/Neo4j/Qdrant.
* Ensure that `Wit<Instant>` is fed only when it has sufficient `Sensation` inputs — fail early otherwise.
* Be mindful of the single-CPU assumption — prefer concurrency without heavy parallelism.
* When skipping speech for empty responses, increment the turn counter so the conversation loop can exit.
* Log Coqui TTS request URLs with `info!(%url, "requesting TTS")` to ease debugging misconfigured endpoints.
* Log each Wit tick with its name and keep loops alive even when idle.
* Log Ollama prompts and streamed chunks with `debug!` for troubleshooting.
* When introducing new CLI arguments or environment variables, update
  `.env.example` and README examples accordingly.

This document reflects the current cognitive and runtime architecture of Pete Daringsby. Keep it consistent with the latest design discussions and behavior changes.

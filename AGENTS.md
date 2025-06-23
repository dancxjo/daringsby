# Agent Instructions

This repository is a Rust workspace.

## Setup

* Install the stable Rust toolchain.
* Ensure the `rustfmt` and `clippy` components are installed.
* Run `cargo fetch` to warm the cache before testing.

## Running & Testing

* Run tests with `cargo test` from the repository root.
* Some tests check debug logs; set `RUST_LOG=debug` when running them.
* Format with `cargo fmt`.
* Use `tracing` macros for all logging.
* Initialize logging in binaries with `tracing_subscriber::fmt::init()`.

## Project Layout

* Crate `pete` depends on local crate `psyche`.
* Crates should be logically modular; split files beyond \~200 lines.

## Code Practices

* Prefer traits for abstraction (`Mouth`, `Ear`, `Wit`).
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
* Track spawned task `JoinHandle`s in a `TaskGroup` so `drop` aborts them.
* `lingproc::push_prompt_context` and `lingproc::take_prompt_context` manage
  temporary prompt notes. `Chatter::update_prompt_context` uses them by
  default to append text to future prompts.

## Frontend

Static assets live under `frontend/dist` and are served by the `pete` binary.
Navigate to `http://localhost:3000/` to load the web face which connects to
`ws://localhost:3000/ws`.

### Frontend Notes

The previous Deno-based client has been removed. Update the files in
`frontend/dist` directly to change the interface.
* Queue audio playback on the client so clips never overlap.
* Define CSS variables in `styles.css` to control colors and fonts.
* Import web fonts in `index.html` and assign them via the `--font-family` CSS variable.
* Reuse a single `<audio>` element for speech playback so controls remain visible.
* After playing speech audio, send an `Echo` message with the spoken text so the conversation log records assistant dialogue.
* Define CSS variables in `styles.css` to control colors and fonts.
* Keep the thought bubble hidden until there is text to display.
* Style the thought bubble with cloud-like lobes and center-bottom connectors.
* When updating the conversation log, preserve scroll position unless the user
  was already at the bottom.
* Serve over HTTPS by passing `--tls-cert` and `--tls-key` to the `pete` binary.
* Canvas elements that repeatedly call `getImageData` must obtain their context
  with `{ willReadFrequently: true }`.

## Communication

* Expose WebSocket chat at `/ws`, forwarding all `Psyche` events.
* Debug information from Wits streams via `/debug`.
* The `EventBus` retains the most recent `WitReport` for each Wit so new
  `/debug` subscribers see existing output immediately.
* SSE endpoints like `/chat` are deprecated; use WebSocket only.
* Text messages no longer trigger `Heard` responses.

## Audio / TTS

* The `tts` feature streams audio from Coqui TTS.
* Configure with `--tts-url` CLI flag.
* Build the `pete` binary with `--features tts` to enable audio.
* Stub TTS in tests to avoid delays.
* Include the `style_wav` parameter when calling Coqui TTS. Use an empty value
  if no style WAV is configured.
* Speech is emitted via `Event::Speech { text, audio }`.

## Specialized Notes

* `Wit` runs asynchronously and infrequently — do not block main loop. Implement
  tick-based summarization when possible.
* Voice should **only** generate dialogue; all decisions routed through `Will`.
* Memory graph (Neo4j) and embedding DB (Qdrant) must stay in sync.
* Long-lived memories are stored as `Experience<T>` combining an `Impression<T>` with an embedding and id.
* Summarizing Wits should handle their own buffering; the `Prehension` wrapper has been removed.
* When adding new Wits that emit `WitReport`s, prefer constructors like
  `with_debug` and register them with `psyche.wit_sender()` so debug output is
  available during tests.
* Tests expecting a `WitReport` must enable the matching debug label with
  `psyche::debug::enable_debug(label).await`.
* `Psyche` uses `active_experience_tick` when speaking to process sensations more frequently.

## Contributor Notes

* Use meaningful commit messages.
* Keep README examples up to date with public APIs.
* Document all new CLI arguments and environment flags.
* Avoid `echo $?`; rely on return values/output checks.
* Favor TDD/BDD when adding features; write failing tests first.
* Provide stub implementations for external ML components so tests run offline.
* `FaceSensor` uses `DummyDetector` for tests; real detectors may require OpenCV.
* `FaceSensor` caches the last embedding to avoid redundant vectors.
* There is no bundled frontend. Connect your own WebSocket client to
  `ws://localhost:3000/ws`.
* Give every new Wit a `LABEL` constant and a `with_debug` constructor for emitting `WitReport`s.
* Re-export shared structs rather than defining duplicates across modules.

## LLM Integration

* Fast LLMs (e.g. Ollama) for `Will`, `Voice`, `Combobulator`.
* Slow/idle LLMs for `Memory`, `Narrator`.
* Only `Will` may invoke `Voice::take_turn`.
* `Voice::take_turn` extracts emoji and emits `Event::EmotionChanged`.
* `Voice` will not speak until `Will::command_voice_to_speak` grants permission.
* `Voice::permit` is idempotent and returns early when already ready.
* `Will::tick` may call `voice.permit(Some(prompt))` to trigger speech when rules allow.

## Additional Suggestions

* Consider adding unit tests that simulate full conversation loops (with mocked `Mouth`, `Ear`, `Voice`).
* Consider adding CLI test scaffolding for mocking TTS/Neo4j/Qdrant.
* Be mindful of the single-CPU assumption — prefer concurrency without heavy parallelism.
* When skipping speech for empty responses, increment the turn counter so the conversation loop can exit.
* Log Coqui TTS request URLs with `info!(%url, "requesting TTS")` to ease debugging misconfigured endpoints.
* Log each Wit tick with its name and keep loops alive even when idle.
* Log Ollama prompts and streamed chunks with `debug!` for troubleshooting.
* Log all LLM prompts and final responses to stdout using `tracing` macros.
* When introducing new CLI arguments or environment variables, update
  `.env.example` and README examples accordingly.
* Log unknown sensation types in `Quick::describe` to surface missing
  downcasts.
* Use `#[tokio::test(start_paused = true)]` and `tokio::time::advance` for
  timeout-related tests to avoid slow sleeps.
* Deduplicate buffering logic when handling voice sensations to prevent
  duplicate entries.
* Extract repeated asynchronous loops into helper functions to reduce
  duplication.

### Quick

The Quick is Pete’s first-stage integrator. It buffers raw `Sensation`s over a short window (a few seconds) and emits an `Instant` — a coherent, narrative `Impression` of what Pete just experienced.

- Input: `Sensation` (from webcam, mic, face detector, etc.)
- Output: `Instant` (e.g., “I hear Travis say, 'Hiya Pete.'”)
- Consumed by: `Will`, `Memory`, `Heart`

🧠 The Quick does **not** act — it observes and narrates.

### Will

The Will interprets `Instant` impressions from the Quick and decides how Pete
should respond. It does not generate new impressions itself. Instead, it emits
behavioral tags like `<say>` or custom motor commands. If the Quick reports
"I'm seeing a fly quickly approach me and then hesitate", the Will might choose
to send `<pounce target="fly">Now or never!</pounce>`.

This document reflects the current cognitive and runtime architecture of Pete Daringsby. Keep it consistent with the latest design discussions and behavior changes.

## Sensor Features

* Build with cargo features to include sensors.
* Features: `eye`, `face`, `geo`, `ear`, `heartbeat`.
* `all-sensors` enables them all and is used by default.
* `HeartbeatSensor::test_interval` helps with short test delays.
\n* In doctests, use `crate::` paths to reference items within the same crate.

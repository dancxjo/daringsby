# Pete Daringsby Agent Overview

This document is for contributors, agents, and automated tools working on the Pete Daringsby project. It provides a high-level orientation to the structure, roles, and behaviors of the system.

## 🧠 Core Concept

Pete is a narratively coherent artificial agent built in Rust. He perceives the world through sensors, forms internal impressions, reflects on meaning, chooses actions, and expresses himself. His architecture is modular and layered to support different cognitive roles.

---

## 🧩 System Architecture

### `psyche` (Core Cognition)

Responsible for Pete's internal thinking and memory.

* **Wits**: Modular units of cognition.

  * `Quick`: Integrates raw `Sensation`s into timestamped `Stimulus` entries and emits an immediate `Impression`.
  * `Combobulator`: Summarizes immediate `Impression`s into broader awareness.
  * `Memory`: Stores remembered `Experience<T>` records in Neo4j and Qdrant.
  * `Heart`: Detects emotional tone (emoji) from recent experience.
  * `Will`: Chooses actions based on situation, emits chat-formatted responses with `<thought>` tags, and tagged commands (e.g. `<pounce>`).
  * `Voice`: Generates natural language dialogue (when permitted).

* **Other Components**:

  * `FondDuCoeur`: Core identity narrative (evolving paragraph).
  * `Conversation`: Stores the back-and-forth chat.
  * `EventBus`: Publishes `Event` and `WitReport` to observers.
  * `PromptBuilder`: Helper trait for constructing LLM prompts.

### `pete` (Host Implementation)

Wires sensors and actuators to the cognitive engine.

* **Sensors**: `EyeSensor`, `GeoSensor`, `HeartbeatSensor`, etc.
* **Mouths**: `ChannelMouth`, `TtsMouth`, `NoopMouth`
* **Ears**: `ChannelEar`, `NoopEar`
* **Motors**: Logging, simulated, or real-world actions
* **Web Interface**: Serves static frontend and `/ws` WebSocket
* **`Body` struct**: Holds live connection between frontend and `Psyche`

### `lingproc` (Language Providers)

Provides LLM and embedding utilities.

* **LLM Traits**: `Chatter`, `Doer`, `Vectorizer`
* **OllamaProvider**: Backend for generation and embedding
* Vectorizers should warn if no embeddings are returned to avoid silent similarity errors
* **Helpers**: Sentence segmentation, prompt context, instruction parsing

---

## 🔄 Cognitive Flow

1. **Perception**

   * Sensors emit `Sensation`s to the `Psyche`.
   * `Quick` converts them into an immediate `Impression`.

2. **Integration**

   * `Combobulator` processes a compressed timeline of concurrent events to describe the current situation in a single sentence.
   * `Memory` links this impression to past context, updating long-term memory.

3. **Emotion**

   * `Heart` derives an emoji-represented emotional state.
   * This is passed to the frontend for display.

4. **Will & Action**

   * `Will` considers the current situation and emotional tone.
   * May emit behavioral instructions (e.g., `<say>`, `<pounce>`, `<move>`).
   * The standalone `will` binary also reads the latest combobulation without
     a timeline and generates a chat-formatted response with `<thought>` tags and an emoji. It records
     `I think: ...`, `I ought to say: ...`, and `I turn my face into a $EMOJI.` as impression sensations.

5. **Speech**

   * `Will` invokes `Voice::take_turn()` with a prompt, permitting it to speak.
   * `Voice` emits structured speech and updates the conversation log.

6. **Face Expression**

   * The `face` binary still mirrors direct combobulation emojis to `/ws`.
   * It also treats Will's `I turn my face into...` impressions as presentable
     face expressions.
   * Every emitted face emoji is redundantly stored as
     `I feel my face turn into a $EMOJI.` This lets the graph preserve both
     pseudoconscious facial control from Will and microexpression-like shifts
     from combobulation.

---

## 💬 Communication Channels

* **WebSocket** at `/ws`: Streams `Event` objects from Pete to the client
* **Static Frontend**: Lives under `frontend/dist`; connects to `/ws`
* **Events**: Include `Sensed`, `Spoke`, `EmotionChanged`, `Speech`, etc.
* **Debug Panel**: `WitReport` events are delivered on `/ws` as `Think` messages

---

## 🧪 Testing Practices

* Use `#[tokio::test(start_paused = true)]` for time-sensitive async tests
* Simulate full cognition loops with stubbed `Mouth`, `Ear`, and LLM
* Enable `tts` feature for Coqui integration, or test without it
* Avoid blocking: all Wits run asynchronously and should tick infrequently
* Implement simple buffer-based Wits using `BufferedWit` to avoid duplicating
  `tick`/`observe` boilerplate
* Assign stable `id` attributes to dynamic DOM nodes in `frontend/dist/app.js`
  to simplify e2e tests

---

## 🧠 Agent Roles Summary

| Agent          | Role                                       |
| -------------- | ------------------------------------------ |
| `Quick`        | Turns `Sensation[]` into an immediate `Impression` |
| `Combobulator` | Describes the moment in a single sentence  |
| `Memory`       | Stores, links, and recalls impressions     |
| `Heart`        | Assesses how Pete feels                    |
| `Will`         | Chooses and emits actions                  |
| `Voice`        | Generates expressive language (with emoji) |

---

## 🛠 Development Quickstart

* `cargo fetch` then `cargo test`
* `npm test` to run frontend unit tests
* Run with `RUST_LOG=debug cargo run --features tts`
* Visit [`http://localhost:3000/`](http://localhost:3000/) to connect frontend
* Each Wit exposes `new()` and `with_debug()`; `new` should delegate to
  `with_debug` with `None` so devtools can uniformly enable debug output
* Document intentionally empty trait methods with comments so their purpose is
  clear.
* Consolidate common math helpers like `cosine_similarity` in the `common` crate
  and re-export them from other crates to avoid duplication.
* Keep commit messages concise and use tests to drive development (TDD/BDD).
* Reuse cargo and npm caches when running tests to avoid re-downloading
  dependencies.
* Keep commit messages short yet descriptive.
* In frontend scripts, stop `MediaRecorder` on `window.onbeforeunload` to release the microphone.
* Patch DOM incrementally or debounce updates instead of replacing innerHTML.
* Give `<details>` elements a `min-height` and manage `max-height` via the
  `--details-max-height` CSS variable so collapsed summaries remain visible.
* Guard WebSocket sends with `readyState` checks and wait for an open connection
  before starting sensors like the webcam or microphone.
* Restart the webcam if its stream ends by listening for the track's `ended`
  event and reacquiring the camera.
* Avoid reacquiring the webcam when an active stream is already running.
* Start capturing only once `webcamReady` is set after the WebSocket `open`
  event.


## 📝 Coding Guidelines

* When exposing items from a submodule, prefer `pub use` with a private `mod`.
  Use `pub mod` alongside `pub use` only when external crates rely on paths like
  `psyche::module::Item`, and add a comment explaining the duplication.

Use this document to orient new agents, tools, or contributors. If you’re confused — ask the Quick what it saw, or the Will what it wants.

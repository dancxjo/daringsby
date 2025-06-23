# Pete Daringsby Agent Overview

This document is for contributors, agents, and automated tools working on the Pete Daringsby project. It provides a high-level orientation to the structure, roles, and behaviors of the system.

## 🧠 Core Concept

Pete is a narratively coherent artificial agent built in Rust. He perceives the world through sensors, forms internal impressions, reflects on meaning, chooses actions, and expresses himself. His architecture is modular and layered to support different cognitive roles.

---

## 🧩 System Architecture

### `psyche` (Core Cognition)

Responsible for Pete's internal thinking and memory.

* **Wits**: Modular units of cognition.

  * `Quick`: Integrates raw `Sensation`s into a coherent `Instant`.
  * `Combobulator`: Summarizes the present `Instant` into an `Impression`.
  * `Memory`: Stores `Experience<Impression>` in Neo4j and Qdrant.
  * `Heart`: Detects emotional tone (emoji) from recent experience.
  * `Will`: Chooses actions based on situation, emits tagged commands (e.g. `<pounce>`).
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
* **Helpers**: Sentence segmentation, prompt context, instruction parsing

---

## 🔄 Cognitive Flow

1. **Perception**

   * Sensors emit `Sensation`s to the `Psyche`.
   * `Quick` converts them into an `Instant`.

2. **Integration**

   * `Combobulator` describes the current situation in a single sentence.
   * `Memory` links this impression to past context, updating long-term memory.

3. **Emotion**

   * `Heart` derives an emoji-represented emotional state.
   * This is passed to the frontend for display.

4. **Will & Action**

   * `Will` considers the current situation and emotional tone.
   * May emit behavioral instructions (e.g., `<say>`, `<pounce>`, `<move>`).

5. **Speech**

   * `Will` invokes `Voice::take_turn()` with a prompt, permitting it to speak.
   * `Voice` emits structured speech and updates the conversation log.

---

## 💬 Communication Channels

* **WebSocket** at `/ws`: Streams `Event` objects from Pete to the client
* **Static Frontend**: Lives under `frontend/dist`; connects to `/ws`
* **Events**: Include `Sensed`, `Spoke`, `EmotionChanged`, `Speech`, etc.
* **Debug Panel**: Streams `WitReport`s via `/debug`

---

## 🧪 Testing Practices

* Use `#[tokio::test(start_paused = true)]` for time-sensitive async tests
* Simulate full cognition loops with stubbed `Mouth`, `Ear`, and LLM
* Enable `tts` feature for Coqui integration, or test without it
* Avoid blocking: all Wits run asynchronously and should tick infrequently
* Implement simple buffer-based Wits using `BufferedWit` to avoid duplicating
  `tick`/`observe` boilerplate

---

## 🧠 Agent Roles Summary

| Agent          | Role                                       |
| -------------- | ------------------------------------------ |
| `Quick`        | Turns `Sensation[]` into an `Instant`      |
| `Combobulator` | Describes the moment in a single sentence  |
| `Memory`       | Stores, links, and recalls impressions     |
| `Heart`        | Assesses how Pete feels                    |
| `Will`         | Chooses and emits actions                  |
| `Voice`        | Generates expressive language (with emoji) |

---

## 🛠 Development Quickstart

* `cargo fetch` then `cargo test`
* Run with `RUST_LOG=debug cargo run --features tts`
* Visit [`http://localhost:3000/`](http://localhost:3000/) to connect frontend

Use this document to orient new agents, tools, or contributors. If you’re confused — ask the Quick what it saw, or the Will what it wants.

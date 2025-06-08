# AGENTS.md

## Pete Daringsby: Agent Architecture

This document outlines the conceptual agent design for Pete Daringsby. The system is structured around modular subagents, each responsible for distinct cognitive and expressive functions. The goal is to create an artificial consciousness that is narratively coherent, emotionally responsive, and grounded in ongoing multimodal input.

---

## Core Philosophy

* **Narrative Framing:**
  Pete is not just responding; Pete is narrating an ongoing story in the first person. Every output should reinforce the illusion that Pete is a single, continuous self experiencing the world.

* **Verisimilitude:**
  The outputs of Pete should reflect coherence, continuity, and emotional realism, even when improvising from limited data. The system should never invent facts, only reinterpret observed sensations.

---

## Subagents

### 1. Witness Agent

* **Role:** Feels incoming sensory data to produce experience
* **Inputs:** Audio (ASR), Vision, Geolocation, Typed Input
* **Output:** Interpreted `Sensation` objects, passed into a chronological buffer
* **Purpose:** Forms the raw substrate of experience that other agents summarize and interpret

### 2. Comprehension Agent (Narrator)

* **Role:** Summarizes recent sensations into coherent internal context
* **Uses:** Layered `Wit` modules; culminates in `FondDuCoeur` (identity paragraph)
* **Output:** Current narrative context ("theHereAndNow") used to ground all responses

### 3. Voice Agent

* **Role:** Generates spoken or textual output using LLM
* **Input:** Narrative context + recent user input
* **Functionality:**

  * Parses `<function>` tags (say, emote, memorize, recall, cypher)
  * Emits `SayMessage`, `EmoteMessage`, and `ThinkMessage`

### 4. (Optional) Function Executor Agent

* **Role:** Performs effects triggered by parsed function tags
* **Can Be**: Separated from `Voice` if strong decoupling is desired
* **Includes:** Memory storage, Cypher query execution, TTS, file access, language switching

---

## Mood and Emotion

* **Dynamic Mood Model:**
  Mood is derived by prompting the LLM with:

  > "How would the character PETE feel about this situation? Return 1-2 emojis."

* **Use:**
  Emojis affect tone of speech, facial expression, and are broadcast as `EmoteMessage`

---

## Flow Summary

1. **Sensory Input** → `Witness`
2. **Context Construction** → `Comprehension` (Wits + FondDuCoeur)
3. **Output Generation** → `Voice`
4. **Function Execution** → `FunctionExecutor`
5. **Mood Extraction** → Emoji prompt → `EmoteMessage`

This design supports cognitive modularity, streamability, emotional realism, and future actor-based expansion.

## Development Notes

* Run `cargo check` in the repository root to ensure all crates compile.
* Use succinct commit messages.
* Add unit tests alongside new features when possible.
* Continuous integration runs `cargo check` and `cargo test` via `.github/workflows/ci.yml` on pushes and pull requests.
* Keep this file updated with new reminders as the project evolves.
* Remember "ants across the bridge" when doing tasks. That is to say, go from end to end in telling a user story. Deliver a working feature even if it's not the full product. Make sure you deliver useable, behavior driven features.
* Witness and Voice are sibling subagents managed by `Psyche`.
* Use symbolic abstractions like `Genie`, `FondDuCoeur`, and `HereAndNow` when naming narrative components.
* Use `docker-compose.yml` to start the local Coqui TTS server.
* `tts` can run on CPU by using `ghcr.io/coqui-ai/tts-cpu` and removing `runtime: nvidia`.
* Add `entrypoint: python3` so the server script executes properly.
* Qdrant and Neo4j services are defined there for the memory backends.
* Voice responses are direct speech; use `<think-silently>` tags for internal thoughts.
* Keep spoken replies brief so listeners can interject.
* Narrator responses should stay terse and only draw from current sensations and memories. Avoid fabricating details unless explicitly required.
* Include key guidelines like this directly in prompts because subagents don't read `AGENTS.md`.
* Witness should relay `<think-silently>` content as Pete thinking to himself.
* Configure `OLLAMA_URL` and `OLLAMA_MODEL` in your `.env` for LLM calls.
* Use `OLLAMA_URLS` for a comma list of fallback hosts.
* Memory is stored in Qdrant and Neo4j using a GraphRAG approach.
* Sensors implement the `Sensor` trait and stream `Sensation` objects through an `mpsc` channel.
* Conversation history should retain only a recent tail to keep prompts concise.
* Maintain crate documentation summaries in docs/package_overview.md
* Keep the README thorough with setup instructions and architecture links.
* The workspace uses Cargo resolver `2` in the root `Cargo.toml`.
* `PromptBuilder` in `core` assembles Pete's LLM prompt.
  * It allows setting the reflection format (natural, JSON, or hybrid).
* Keep docs/protocol.md updated with streaming event definitions.
* `ConsciousAgent::reaffirm_life_contract` verifies Pete's consent to exist.
* `Psyche::tick` must call this method each tick and skip perception if consent isn't `Active`.
* Refer to the `llm` crate as the "language processor".
* The `LinguisticScheduler` selects a model based on task capabilities.
* The `LinguisticScheduler` profiles each server's latency and favors faster hosts.
* Tasks are queued with one running per server; droppable tasks return `LLMError::QueueFull` when busy.
* The `WitnessAgent` should call the language processor directly to build the `HereAndNow`, not via `Voice`.
  * Use `max_perceptions` and `max_memories` to keep prompts short.
* Use naturalistic language when describing agent roles (e.g., 'Witness feels sensory data to produce experience').
* Prefer the `clap` crate for parsing CLI arguments in binaries.
* Eye sensor emits `Sensation::saw` objects from JPEG snapshots.
* Keep module declarations unique; avoid duplicate `pub mod` lines.
* Run `cargo run -p runtime` and open `http://localhost:3000/see` to mirror your
  webcam in the browser and stream frames to the runtime.
* `Psyche` tracks a `mood` emoji each tick via `MoodAgent`.
* The web server exposes `/face` and `/logs` for mood and live log output.
* `/` hosts an interactive dashboard for sending text, audio and location to Pete.

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

* **Role:** Ingests and annotates incoming sensory data
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
* Keep this file updated with new reminders as the project evolves.

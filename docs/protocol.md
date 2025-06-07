# WebSocket Streaming Protocol

This document defines the event types used when streaming data between Pete Daringsby's components. Each message is sent as JSON over WebSocket.

## Event Types

- `asr_partial` – interim transcript fragment from the microphone.
- `asr_final` – finalized transcript for a sentence.
- `llm_thought_fragment` – partial LLM reflection text.
- `llm_final_response` – complete LLM response for a sentence.
- `tts_chunk_ready` – identifier for an audio chunk ready to play.
- `perception_log` – log message from Witness or other sensors.
- `memory_update` – summary of an Experience stored in memory.
- `consent_check` – result of reaffirming the life contract.
- `vision_description` – first-person text of what Pete sees.

Multiple clients can subscribe to these events. Each client maintains its own state so that perception and responses remain isolated.


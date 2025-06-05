# Pete Daringsby

This repository implements a multi‑modal chat system built with [Fresh](https://fresh.deno.dev/) and Deno. A browser client connects to the server over WebSockets and streams user input such as text, geolocation and webcam snapshots. The backend uses several language model driven modules to respond with speech, emoji expressions and thought updates.

## Features

- **WebSocket client** written with Preact signals that captures geolocation, webcam images and textual input.
- **Psyche** orchestrator that manages sessions and distributes sensory events to the rest of the system.
- **Voice** module that generates replies using language models (via Ollama), records conversations in Neo4j, and can run inline TypeScript, fetch requests and Cypher queries.
- **Contextualizer** loads relevant memories from Neo4j and Qdrant to provide context for each reply.
- **ImageDescriber** converts webcam snapshots to first‑person observations via a vision‑enabled model.
- **Audio synthesis** through an external TTS service with playback queued in the browser.
- Docker environment that sets up Nginx, Neo4j, Qdrant, a TTS server and Whisper.

## Running locally

### With Deno
```bash
deno task start
```
This starts the development server on port 8000. Required services should be running and environment variables such as `OLLAMA_URL`, `NEO4J_URL` and `QDRANT_URL` need to be configured.

### With Docker Compose
```bash
docker-compose up
```
This command launches the entire stack defined in `docker-compose.yml`, including supporting services and an Nginx reverse proxy. Self‑signed certificates are created on startup.

## Repository structure

- `routes/` – Fresh route handlers and the WebSocket endpoint.
- `islands/` – Interactive browser components for webcam, geolocation and text input.
- `lib/` – Server modules such as `psyche`, `voice`, networking utilities and vision helpers.
- `static/` – Styles and assets served to the client.

## Contributing

Contributions are welcome. Feel free to open issues or pull requests.

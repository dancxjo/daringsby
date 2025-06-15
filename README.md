# Daringsby Workspace

This repository contains a Rust workspace with two crates:

- **psyche** – a library crate providing the `Psyche` type
- **ling** – helper LLM abstractions exposed through the `psyche` crate
- **pete** – a binary crate depending on `psyche`

Example with the `OllamaProvider`:

```rust,no_run
use psyche::ling::OllamaProvider;
use psyche::Psyche;

let narrator = OllamaProvider::new("http://localhost:11434", "mistral").unwrap();
let voice = OllamaProvider::new("http://localhost:11434", "mistral").unwrap();
let vectorizer = OllamaProvider::new("http://localhost:11434", "mistral").unwrap();
let psyche = Psyche::new(Box::new(narrator), Box::new(voice), Box::new(vectorizer));
psyche.run().await;
```


Run tests with:

```sh
cargo test
```

Run the web server with:

```sh
cargo run -p pete
```
You can POST JSON `{ "message": "hi" }` to `http://127.0.0.1:3000/chat` and receive an SSE stream in response.

## Web Interface

After starting the server, visit `http://127.0.0.1:3000/` in your browser. The page connects to `ws://localhost:3000/ws` and lets you chat with Pete in real time.
The same events are also available via server-sent events at `POST /chat`.

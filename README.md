# Daringsby Workspace

This repository contains a Rust workspace with three crates:

- **psyche** – a library crate providing the `Psyche` type
- **lingproc** – helper LLM abstractions re-exported by `psyche`
- **pete** – a binary crate depending on `psyche`

`Psyche` starts with a prompt asking the LLM to respond in one or two sentences at most. You can override it with `set_system_prompt`.

Example with the `OllamaProvider`:

```rust,no_run
use psyche::ling::OllamaProvider;
use psyche::Psyche;

let narrator = OllamaProvider::new("http://localhost:11434", "mistral").unwrap();
let voice = OllamaProvider::new("http://localhost:11434", "mistral").unwrap();
let vectorizer = OllamaProvider::new("http://localhost:11434", "mistral").unwrap();
use psyche::{Ear, Mouth};
use async_trait::async_trait;

struct DummyMouth;
#[async_trait]
impl Mouth for DummyMouth {
    async fn speak(&self, _t: &str) {}
    async fn interrupt(&self) {}
    fn speaking(&self) -> bool { false }
}

struct DummyEar;
#[async_trait]
impl Ear for DummyEar {
    async fn hear_self_say(&self, _t: &str) {}
    async fn hear_user_say(&self, _t: &str) {}
}

let psyche = Psyche::new(
    Box::new(narrator),
    Box::new(voice),
    Box::new(vectorizer),
    std::sync::Arc::new(DummyMouth),
    std::sync::Arc::new(DummyEar),
);
// replace the dummy mouth with your own implementation
let mouth = std::sync::Arc::new(DummyMouth);
psyche.set_mouth(mouth);
// Customize or replace the default prompt if desired
psyche.set_system_prompt("Respond with two sentences.");
psyche.set_echo_timeout(std::time::Duration::from_secs(1));
// make Pete wait for you to speak first
psyche.set_speak_when_spoken_to(true);
psyche.run().await;
assert!(!psyche.speaking());
```


Run tests with:

```sh
cargo test
```

Run the web server with the built-in Ollama support:

```sh
cargo run -p pete -- --ollama-url http://localhost:11434 --model mistral
```
## Web Interface

After starting the server, visit `http://127.0.0.1:3000/` in your browser. The page connects to `ws://localhost:3000/ws` and lets you chat with Pete in real time.
When the page receives a `pete-says` message it echoes back `{type: "displayed", text}` so the server knows the line was shown.

The page also opens `ws://localhost:3000/log` to stream server logs. Connection statuses for both sockets are shown in the sidebar.

### Logging

Set `RUST_LOG=info` when running the server to enable helpful tracing output.

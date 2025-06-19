# Daringsby Workspace

This repository contains a Rust workspace with three crates:

- **psyche** – a library crate providing the `Psyche` type
- **lingproc** – helper LLM abstractions re-exported by `psyche`
- **pete** – a binary crate depending on `psyche`

The `psyche` crate also defines a `Summarizer` trait used to build modular
cognitive layers. Each `Summarizer` asynchronously digests a batch of lower
level impressions and produces a higher-level `Impression<T>`. A lightweight
`Wit<I, O>` trait is available for incrementally observing inputs and emitting
periodic impressions of type `O`. The `Prehension` helper buffers incoming
impressions and summarizes them using a `Summarizer`.

`Psyche` starts with a prompt asking the LLM to respond in one or two sentences at most. You can override it with `set_system_prompt`.
Pete's mouth streams audio one sentence at a time so long replies don't block.

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

struct DummyVoice;
#[async_trait]
impl psyche::ling::Chatter for DummyVoice {
    async fn chat(&self, _s: &str, _h: &[psyche::ling::Message]) -> anyhow::Result<psyche::ling::ChatStream> {
        Ok(Box::pin(tokio_stream::once(Ok("😊".to_string()))))
    }
}

let psyche = Psyche::new(
    Box::new(narrator),
    Box::new(voice),
    Box::new(vectorizer),
    std::sync::Arc::new(psyche::NoopMemory),
    std::sync::Arc::new(DummyMouth),
    std::sync::Arc::new(DummyEar),
);
// replace the dummy mouth with your own implementation
let speaking = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
let display = std::sync::Arc::new(pete::ChannelMouth::new(psyche.event_sender(), speaking.clone()));
#[cfg(feature = "tts")]
let tts = std::sync::Arc::new(psyche::PlainMouth::new(
    std::sync::Arc::new(pete::TtsMouth::new(
        psyche.event_sender(),
        speaking.clone(),
        std::sync::Arc::new(pete::CoquiTts::new(
            "http://localhost:5002/api/tts",
            Some("p376".into()),
            None,
        )),
    )) as std::sync::Arc<dyn Mouth>
));
#[cfg(feature = "tts")]
let mouth = std::sync::Arc::new(psyche::AndMouth::new(vec![display.clone(), tts]));
#[cfg(feature = "tts")]
let mouth = std::sync::Arc::new(psyche::TrimMouth::new(mouth));
#[cfg(not(feature = "tts"))]
let mouth = display.clone() as std::sync::Arc<dyn Mouth>;
let mouth = std::sync::Arc::new(psyche::TrimMouth::new(mouth));
psyche.set_mouth(mouth);
psyche.set_emotion("😊"); // initial expression
// Ask the Will what to do next
let will = psyche::Will::new(Box::new(DummyVoice));
let decision = will
    .digest(&[psyche::Impression { headline: "".into(), details: None, raw_data: "say hi".to_string() }])
    .await?;
assert_eq!(decision.headline, "Speak.");
will.command_voice_to_speak(None); // allow Pete to respond
// Build a custom instruction with the prompt generator
let custom = psyche::WillPrompt::default().build("say hi");
assert!(custom.contains("Pete"));
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

To enable audio output via Coqui TTS, build with the optional `tts` feature and
provide the TTS server URL and optional voice parameters:

```sh
cargo run -p pete --features tts -- \
  --ollama-url http://localhost:11434 --model mistral \
  --tts-url http://localhost:5002/api/tts \
  --tts-speaker-id p376
```
## Web Interface

After starting the server, visit `http://127.0.0.1:3000/` in your browser. The page connects to `ws://localhost:3000/ws` and lets you chat with Pete in real time.
A second WebSocket at `ws://localhost:3000/debug` streams debugging information from the Wits.
When the page receives a `pete-says` message it echoes back `{type: "displayed", text}` so the server knows the line was shown. Connection status is shown in the sidebar.
Pete conveys emotion directly in responses using emoji.
Emotion updates arrive via `pete-emotion` messages containing an emoji string.

Fetch the raw conversation log at `/conversation`:

```sh
curl http://127.0.0.1:3000/conversation
```

Which returns JSON like:

```json
[{"role":"user","content":"Hi"}]
```

### Logging

Set `RUST_LOG=info` when running the server to enable helpful tracing output.

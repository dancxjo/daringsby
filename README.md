# Daringsby Workspace

This repository contains a Rust workspace with three crates:

- **psyche** – a library crate providing the `Psyche` type
- **lingproc** – helper LLM abstractions
- **pete** – a binary crate depending on `psyche`

The `psyche` crate defines a `Summarizer` trait used to build modular
cognitive layers. Each `Summarizer` asynchronously digests a batch of lower
level impressions and produces a higher-level `Impression<T>`. A lightweight
`Wit` trait lets you incrementally observe inputs and emit periodic
impressions using the summarizer implementation. Each implementation specifies
its input and output types via associated `Input` and `Output` types.

The unified cognitive model centers on two types:

* `Stimulus<T>` – any observed item or prior impression with a timestamp.
* `Impression<T>` – interprets stimuli into a summarized thought with an optional emoji.
* `Experience<T>` – a stored impression paired with a vector embedding and unique id.

The first layer, **Quick**, groups raw `Sensation`s from sensors into an `Instant`. Higher Wits such as `Will`, `Memory`, and `Heart` react to these Instants.

`Psyche` starts with a prompt asking the LLM to respond in one or two sentences at most. You can override it with `set_system_prompt`.
Pete's mouth streams audio one sentence at a time so long replies don't block.

Example with the `OllamaProvider`:

```rust,no_run
use lingproc::OllamaProvider;
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
    async fn chat(&self, _s: &str, _h: &[lingproc::Message]) -> anyhow::Result<lingproc::TextStream> {
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
let (bus, _rx) = pete::EventBus::new();
let bus = std::sync::Arc::new(bus);
let display = std::sync::Arc::new(pete::ChannelMouth::new(bus.clone(), speaking.clone()));
#[cfg(feature = "tts")]
let tts = std::sync::Arc::new(psyche::PlainMouth::new(
    std::sync::Arc::new(pete::TtsMouth::new(
        bus.event_sender(),
        speaking.clone(),
        std::sync::Arc::new(pete::CoquiTts::new(
            "http://localhost:5002/api/tts",
            Some("p123".into()),
            Some("en".into()),
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
let bus = psyche::TopicBus::new(8);
let will = psyche::wits::Will::new(bus.clone(), Arc::new(DummyVoice));
will
    .observe(psyche::Impression::new(
        vec![psyche::Stimulus::new("say hi".to_string())],
        "",
        None::<String>,
    ))
    .await;
let decision = will.tick().await.pop().unwrap();
assert_eq!(decision.summary, "Speak.");
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
cargo run -p pete -- \
  --chatter-host http://localhost:11434 --chatter-model mistral \
  --wits-host http://localhost:11434 --wits-model mistral \
  --embeddings-host http://localhost:11434 --embeddings-model mistral \
  --qdrant-url http://localhost:6333 \
  --neo4j-uri bolt://localhost:7687 \
  --neo4j-user neo4j \
  --neo4j-pass password

To enable audio output via Coqui TTS, build with the optional `tts` feature and
provide the TTS server URL and optional voice parameters:

```sh
cargo run -p pete --features tts -- \
  --chatter-host http://localhost:11434 --chatter-model mistral \
  --wits-host http://localhost:11434 --wits-model mistral \
  --embeddings-host http://localhost:11434 --embeddings-model mistral \
  --qdrant-url http://localhost:6333 \
  --neo4j-uri bolt://localhost:7687 \
  --neo4j-user neo4j \
  --neo4j-pass password \
  --tts-url http://localhost:5002/api/tts \
  --tts-speaker-id p123 \
  --tts-language-id en

Use `--auto-voice N` to have Pete speak automatically every N seconds during development.
The default fallback response of "I'm listening." can be disabled with `--no-fallback-turn`.

To serve the interface over HTTPS provide a certificate and key:

```sh
cargo run -p pete -- \
  --chatter-host http://localhost:11434 --chatter-model mistral \
  --wits-host http://localhost:11434 --wits-model mistral \
  --embeddings-host http://localhost:11434 --embeddings-model mistral \
  --tls-cert cert.pem --tls-key key.pem
```
## Web Interface

After starting the server, navigate to `http://localhost:3000/` (or `https://localhost:3000/` when TLS is enabled) to open the built-in web face.
The interface communicates over WebSocket at `ws://localhost:3000/ws` (or `wss://localhost:3000/ws` when using HTTPS).
Another WebSocket at `/debug` streams debugging information from the Wits.
The `/debug/psyche` HTTP endpoint returns JSON with the sensation buffer length
and last tick time for each registered Wit.
Navigate to `/debug/wit/{label}` to view the latest prompt and response for a
specific Wit in real time.
Speech arrives as `say` messages:
```json
{ "type": "say", "data": { "words": "hi", "audio": "UklGRg==" } }
```
Emotion updates arrive via `Emote` messages containing an emoji string:
```json
{ "type": "Emote", "data": "😐" }
```
Debug thoughts are sent as `Think` messages. Connection status is shown in the sidebar.

Fetch the raw conversation log at `/conversation`:

```sh
curl http://127.0.0.1:3000/conversation
```

Which returns JSON like:

```json
[{"role":"system","content":"You are PETE \u2014 ..."}, {"role":"user","content":"Hi"}]
```

### Logging

Set `RUST_LOG=info` when running the server to enable helpful tracing output.

### Simulation Utility

Run `cargo run -p pete --bin simulate -- text "hello"` to send a text message to
the running server. Use the `image` subcommand with a file path to simulate an
image sensation.

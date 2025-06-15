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
psyche.run();
```


Run tests with:

```sh
cargo test
```

Run the program with:

```sh
cargo run -p pete
```

set dotenv-load := true

# Show available commands.
default:
    @just --list

# Start Pete. Extra args are forwarded to the pete binary.
run *args:
    cargo run -p pete --bin pete -- {{args}}

# Start Pete with debug logging unless RUST_LOG is already set.
debug *args:
    RUST_LOG="${RUST_LOG:-debug}" cargo run -p pete --bin pete -- {{args}}

# Fetch all local models, or pass tiny.en/base.en/small.en/URL for Whisper.
fetch model="base.en":
    cargo run -p xtask -- fetch {{model}}

# Compatibility alias for fetching the audio models.
fetch-asr-model model="base.en":
    just fetch {{model}}

# Fetch the default voice embedding model, or pass a custom ONNX URL/filename.
fetch-voice-embedding-model model="":
    cargo run -p xtask -- fetch-voice-embedding-model {{model}}

# Run all Rust and frontend tests.
test: test-rust test-frontend

# Run Rust workspace tests.
test-rust:
    cargo test --workspace

# Run frontend tests.
test-frontend:
    npm test

# Format Rust code.
fmt:
    cargo fmt

# Check Rust formatting.
fmt-check:
    cargo fmt --check

# Check the full Rust workspace.
check:
    cargo check --workspace

# Check Pete with all default features.
check-pete:
    cargo check -p pete

# Check Pete without default features.
check-pete-min:
    cargo check -p pete --no-default-features

# Run clippy across the workspace.
clippy:
    cargo clippy --workspace --all-targets

# Simulate a text input event.
simulate-text text:
    cargo run -p pete --bin simulate -- text "{{text}}"

# Simulate an image input event.
simulate-image path:
    cargo run -p pete --bin simulate -- image "{{path}}"

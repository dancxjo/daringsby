set dotenv-load := true

# Show available commands.
default:
    @just --list

# Start Pete's sensor and offline-processing binaries.
run:
    #!/usr/bin/env bash
    set -euo pipefail

    timestamp="$(date +%Y%m%d-%H%M%S)"
    log_root="${PETE_RUN_LOG_DIR:-logs/run}"
    run_log_dir="${log_root}/${timestamp}"
    mkdir -p "$run_log_dir"
    printf 'writing program logs to %s\n' "$run_log_dir"

    bins=()
    for path in pete/src/bin/*.rs; do
        bin="${path##*/}"
        bin="${bin%.rs}"
        if [[ "$bin" == "forget_silence" ]]; then
            bin="forget-silence"
        fi
        # simulate is an ad hoc client utility that requires a subcommand.
        if [[ "$bin" == "pete" || "$bin" == "simulate" || "$bin" == "raw_retention" ]]; then
            continue
        fi
        bins+=("$bin")
    done
    pids=()

    kill_matches() {
        local pattern="$1"
        local pids
        pids="$(pgrep -f "$pattern" || true)"
        if [[ -n "$pids" ]]; then
            kill $pids 2>/dev/null || true
        fi
    }

    for bin in "${bins[@]}"; do
        kill_matches "cargo run -p pete .*--bin ${bin}([[:space:]]|$)"
        kill_matches "(^|[[:space:]])(.*/)?target/.*/${bin}([[:space:]]|$)"
    done

    sleep 1

    for bin in "${bins[@]}"; do
        pkill -KILL -f "cargo run -p pete .*--bin ${bin}([[:space:]]|$)" 2>/dev/null || true
        pkill -KILL -f "(^|[[:space:]])(.*/)?target/.*/${bin}([[:space:]]|$)" 2>/dev/null || true
    done

    cleanup() {
        if ((${#pids[@]})); then
            kill "${pids[@]}" 2>/dev/null || true
        fi
    }
    trap cleanup INT TERM EXIT

    for bin in "${bins[@]}"; do
        log_file="${run_log_dir}/${bin}.log"
        printf 'starting %-18s -> %s\n' "$bin" "$log_file"
        cargo run -p pete --features scene-vec --bin "$bin" >"$log_file" 2>&1 &
        pids+=("$!")
    done

    set +e
    wait -n "${pids[@]}"
    status="$?"
    set -e
    printf 'a program exited with status %s; logs are in %s\n' "$status" "$run_log_dir"
    exit "$status"

# Forget derived graph/vector data while retaining raw sensations and media.
forget *args:
    cargo run -p pete --bin raw_retention -- --confirm {{args}}

# Start Pete with debug logging unless RUST_LOG is already set.
debug *args:
    RUST_LOG="${RUST_LOG:-debug}" cargo run -p pete --bin pete -- {{args}}

# Fetch all local models, or pass tiny.en/base.en/small.en/large-v3/URL for Whisper.
fetch model="large-v3":
    cargo run -p xtask -- fetch {{model}}

# Compatibility alias for fetching the audio models.
fetch-asr-model model="large-v3":
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

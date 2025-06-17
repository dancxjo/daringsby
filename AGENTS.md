# AGENT Instructions

This repository is now a Rust workspace.

- Install the stable Rust toolchain before running tests.
- Run tests with `cargo test` from the repository root.
- Format with `cargo fmt` when possible.
- Ensure the `rustfmt` component is installed so formatting can run offline.
- Crate `pete` depends on the local `psyche` crate.
- Keep examples and inline docs up to date with code changes.
- Update README examples whenever new public APIs are added.
- When adding binary arguments or library APIs, update tests accordingly.
- Keep `index.html` minimal and updated to connect to `ws://localhost:3000/ws`.
- Display the WebSocket connection status in the page for debugging.
- The chat page uses Alpine.js for binding; preserve this dependency when updating `index.html`.
- Render the chat log as a `<ul>` with `<li>` elements for each message.
- Run `cargo fetch` before testing to warm the cache.
- When embedding `index.html` in the `pete` crate, use `include_str!("../../index.html")`.
- Keep the chat script in `index.html` and `pete/build.rs` in sync.
 - Expose WebSocket chat at `/ws` that forwards psyche events.
 - The server no longer exposes the `/chat` SSE endpoint; real-time events are
   WebSocket-only.
- Use `tracing` macros for all logging.
- Initialize logging in binaries with `tracing_subscriber::fmt::init()`.
- When files grow beyond roughly 200 lines, break them into logical modules.
- Avoid using `echo $?` to verify command success; rely on command output.
- Prefer lightweight test dependencies; stub heavy external services like TTS
  engines to keep CI fast.
- Compose multiple `Mouth` implementations using `AndMouth` when both audio and
  textual output are required.

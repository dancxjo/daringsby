# Pete Binary Notes
- Keep `main.rs` small; move logic to libs.
- Add integration tests under `tests/`.
- Use `warp` for the HTTP/WebSocket server with routes in `web`.
- `cargo test -p pete` before commit.
- Implement external sensors in this crate.
- Import necessary traits (e.g. `psyche::Sensor`) when calling trait methods on
  `Heart`.

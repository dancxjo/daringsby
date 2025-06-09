# Repo Guidelines

## Programmatic Checks
- Run `cargo test --all` before committing when feasible. If the full test
  suite is too slow, run targeted tests (e.g. `cargo test -p <crate>` or
  `cargo test <path>::<test_name>`).

## Development Tips
- During day‑to‑day coding, prefer targeted tests (e.g. `cargo test -p <crate>`)
  to avoid long waits. Save the full `--all` runs for just before a commit.
- Keep dependency caches around between runs to reduce network activity.
- Each crate folder has its own terse `AGENTS.md` with more tips.

## Development
- Follow BDD/TDD principles; add tests alongside new features.
- Use concise commit messages.
- Prefer doc tests and examples for public APIs to aid understanding.
- Log errors instead of silently discarding them.
- When testing streams created with `async_stream`, ensure you poll once more
  after the final item to trigger any cleanup logic.
- When storing timestamped data, prefer field names `when` and `what` for
  clarity.
- Use `how` for the descriptive text inside an `Experience`.
- Each psyche should create its own `EventBus` and web server. Avoid globals.
- Keep `README.md` in sync with `docker-compose.yml` whenever services change.

## Project Overview
Daringsby houses several Rust crates forming a model cognitive system named Pete. Events flow through sensors into a `Heart` of `Wit`s which summarize and store experiences.

### Layout
- `lingproc/` – LLM processors, providers and scheduler
- `modeldb/`  – catalog of available models
- `psyche/`   – sensors, event bus, heart/wit logic and web server
- `memory/`   – graph and vector memory abstractions
- `pete/`     – binary launching the web interface
- After running `cargo fmt`, check `git status` and revert unrelated changes before committing.

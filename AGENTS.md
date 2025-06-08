# Repo Guidelines

## Programmatic Checks
- Run `cargo fmt --all` and `cargo test --all` before committing.

## Development Tips
- During day‑to‑day coding, run `cargo test -p <crate>` and `cargo fmt` on the
  crate you're touching to avoid long waits. Save the full `--all` runs for
  just before a commit.
- Keep dependency caches around between runs to reduce network activity.

## Development
- Follow BDD/TDD principles; add tests alongside new features.
- Use concise commit messages.
- Prefer doc tests and examples for public APIs to aid understanding.
- When testing streams created with `async_stream`, ensure you poll once more
  after the final item to trigger any cleanup logic.

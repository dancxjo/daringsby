# Repo Guidelines

## Programmatic Checks
- Run `cargo test --all` before committing when feasible. If the full test
  suite is too slow, run targeted tests (e.g. `cargo test -p <crate>` or
  `cargo test <path>::<test_name>`).

## Development Tips
- During day‑to‑day coding, prefer targeted tests (e.g. `cargo test -p <crate>`)
  to avoid long waits. Save the full `--all` runs for just before a commit.
- Keep dependency caches around between runs to reduce network activity.

## Development
- Follow BDD/TDD principles; add tests alongside new features.
- Use concise commit messages.
- Prefer doc tests and examples for public APIs to aid understanding.
- When testing streams created with `async_stream`, ensure you poll once more
  after the final item to trigger any cleanup logic.
- When storing timestamped data, prefer field names `when` and `what` for
  clarity.

# AGENT Instructions

This repository contains Deno packages. Install Deno before running tests.

## Testing

Run `deno test` from the repository root. Tests reside in `pete/tests` and
should follow BDD/TDD style using Deno's built-in testing tools.

### Environment setup

If the standard install script is blocked, download the Deno binary from GitHub
releases and place it in `/usr/local/bin`.

### Module layout

`lib.ts` is the main library entry point. `main.ts` demonstrates usage and can
be run with `deno run pete/main.ts`.

### Reminders

- Update tests whenever constructor parameters change, especially for `Psyche`.
- Cache server dependencies with `deno cache server.ts` before tests.
- Keep WebSocket sensor tests in sync with any new event types or message
  flows.
- Ensure `integrate_sensory_input` runs each beat even when speaking. Only
  `take_turn` may be skipped while speech is in progress.
- Index page should log all websocket messages and echo `pete-says` once displayed.

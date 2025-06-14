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
- Keep WebSocket sensor tests in sync with any new event types or message flows.
- Ensure the `quick` Wit processes input each beat even while speaking. Only
  `take_turn` may be skipped while speech is in progress.
- Update tests when adding or modifying Wits.
- Index page should echo `pete-says` once displayed.
- Prompts go to the prompt box and streams go to the stream box.
- Only `pete-says` and user-sent messages belong in the chat log.
- Use Tailwind CSS for styling the index page.
- When using Alpine.js, register listeners in `init()` and mutate state via
  `this` to ensure reactivity.
- Emit a `pete-feels` websocket event whenever Pete's feelings change and
  update tests accordingly.

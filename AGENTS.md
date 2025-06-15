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
- Forward each Wit name with its prompt or streamed chunk so the UI can show a
  tab per Wit.
- Only `pete-says` and user-sent messages belong in the chat log.
- Use Tailwind CSS for styling the index page.
- When using Alpine.js, register listeners in `init()` and mutate state via
  `this` to ensure reactivity.
- Emit a `pete-feels` websocket event whenever Pete's feelings change and update
  tests accordingly.
- Update Autologos sensor tests when output types change.
- Skip `take_turn` when no websocket clients are connected.
- Vary Ollama temperature between 0.7 and 1 on each request.
- When adding a new environment variable, document it in `env.example` and update
  related tests.
- Cache Deno dependencies with `deno cache --lock=deno.lock` when network access is restricted.
- Autologos snippets should include about 10 lines so Pete can read his own code.

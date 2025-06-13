# AGENT Instructions

This repository contains Deno packages. Install Deno before running tests.

## Testing

Run `deno test` from the repository root. Tests reside in `pete/tests` and
should follow BDD/TDD style using Deno's built-in testing tools.

If dependencies are missing, use the official install script or package manager
before running tests. Cache dependencies with `deno cache` to speed up repeated
runs.

If network access is restricted, prefetch dependencies with `deno cache --lock=deno.lock`.
The old `deno vendor` command was removed in Deno 2.

If commands fail due to environment limitations, mention that in the PR's test
results section.

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

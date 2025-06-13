# AGENT Instructions

This repository contains Deno packages. Install Deno before running tests.

## Testing

Run `deno test` from the repository root. Tests reside in `pete/tests` and
should follow BDD/TDD style using Deno's built-in testing tools.

If dependencies are missing, use the official install script or package manager
before running tests. Cache dependencies with `deno cache` to speed up repeated
runs.

If commands fail due to environment limitations, mention that in the PR's test
results section.


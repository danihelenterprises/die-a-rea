# ADR-0015 — Ship a Docker toolchain

- **Status:** Accepted
- **Date:** 2026-10-12
- **Deciders:** die-a-rea maintainers
- **Supersedes:** none

## Context

Contributors, CI and users need a Rust toolchain, Node, and the JS adapter to get
the full pipeline. Installing them locally is friction, and "works on my machine"
breaks reproducibility. The optional adapters (javascript-obfuscator) need a
specific runtime.

## Decision

Provide a multi-stage `docker/Dockerfile` with three targets:

- `build` — compiles the workspace (intermediate).
- `runtime` (default) — the CLI, Node 22, `javascript-obfuscator`, and the
  `direa-js` adapter, with `DIREA_JS_ADAPTER` set.
- `dev` — the runtime plus the Rust toolchain (cargo/rustfmt/clippy) and the
  built CLI, for building, testing and contributing.

The images use `docker/entrypoint.sh`, which is fail-fast: `set -Eeuo pipefail`
plus an `ERR` trap that prints `die-a-rea: error: … (exit N) at line M`. It offers
`help`, `doctor`, `shell`, `test`, `lint`, `fmt`, `release`, passes CLI
subcommands (`plan`/`init`/`build`/`verify`/`profiles`) through to the binary, and
hands unknown commands to the CLI so it prints its own usage. `docker/compose.yml`
wraps the runtime image.

## Consequences

### Positive
- No local install; a reproducible environment for users and CI.
- The JS adapter works out of the box in both images.

### Negative / risks
- Large images and network access at build time (base images, npm).
- Runs as root by default; use `--user` when needed.
- `go`/`garble` are intentionally not installed (bring-your-own).

## Ergonomics

```bash
docker build -f docker/Dockerfile --target runtime -t die-a-rea:runtime .
docker run --rm -v "$PWD":/work die-a-rea:runtime plan
```

Evidence: `docker/Dockerfile`, `docker/entrypoint.sh`, `docker/README.md`,
`docker/compose.yml`; CI `docker` job; verified locally (runtime + dev build,
`doctor`, end-to-end `plan`/`build`/`verify`). Lessons K-021.

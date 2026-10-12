# ADR-0009 — Build executes external adapters opportunistically

- **Status:** Accepted
- **Date:** 2026-10-12
- **Deciders:** die-a-rea maintainers
- **Supersedes:** none

## Context

ADR-0002 defines adapters as subprocesses, but `build` only ran built-ins; the
JavaScript/Electron ecosystem needs Node tooling (`javascript-obfuscator`) that
must not be bundled (size, version drift) or linked. The first real adapter
(B-003) forced the question of how `build` discovers and invokes one.

## Decision

- `build` resolves an external adapter by looking at `DIREA_JS_ADAPTER` (path to
  an executable) and then `direa-js` on `PATH`.
- It runs the adapter with operation `apply` against the **hardened output** in
  the output directory; the source artifact is never touched.
- **Absence or failure of an adapter is a non-fatal note.** The built-in result
  stands and the build stays green.
- Adapters are additive and must never become a hard build dependency.

## Consequences

### Positive
- Optional by design; no new dependency in the core.
- The same mechanism will serve the LLVM/Go/.NET adapters.

### Negative / risks
- Capability varies by host, so results are environment-dependent.
- A measurement may not reflect an adapter's effect until a matching probe
  exists (see K-013 / B-017).

## Ergonomics

Enable with `export DIREA_JS_ADAPTER=/path/adapters/js/direa-js` or by placing
`direa-js` on `PATH`. Evidence: `crates/direa-cli/src/build.rs`
(`resolve_js_adapter`, `apply_js_adapter`); test
`external_adapter_protocol_round_trips`; `adapters/js/direa-js`.

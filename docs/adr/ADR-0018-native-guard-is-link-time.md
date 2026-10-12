# ADR-0018 — The native guard is link-time, not post-build injection

- **Status:** Accepted
- **Date:** 2026-10-12
- **Deciders:** die-a-rea maintainers
- **Supersedes:** none

## Context

B-023 embedded a runtime guard into JavaScript artifacts. Embedding one into an
already-built native binary would require injecting code (new segments, code
caves, entry-point patching) — unsafe, format-specific, and likely to break
signatures and loaders.

## Decision

Ship a small **linkable C library** at `runtime/guard/` (`direa_guard.h`,
`direa_guard.c`, MIT). Program authors compile and link it; `direa_guard_check`
mirrors `direa_core::guard` (TracerPid, process maps, injected-library env) and
calls a weak `direa_guard_on_instrumented` hook. `build` does **not** inject a
guard into native binaries.

## Consequences

### Positive
- No unsafe binary injection; no behaviour change to existing artifacts.
- One portable C file; no dependencies.

### Negative / risks
- It only protects programs built with it; existing binaries are unaffected.
- Detection is heuristic (evadable), like the JS guard.

## Ergonomics

```bash
cc -I runtime/guard app.c runtime/guard/direa_guard.c -o app
```

Evidence: `runtime/guard/direa_guard.{h,c}`; `crates/direa-core/src/guard.rs`
test `native_guard_compiles_and_runs` (compiles and runs the C guard when a C
compiler is present); lesson K-031.

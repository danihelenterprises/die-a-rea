# ADR-0014 — Runtime instrumentation is measured from the environment

- **Status:** Accepted
- **Date:** 2026-10-12
- **Deciders:** die-a-rea maintainers
- **Supersedes:** none

## Context

The `runtime-instrumentation-exposure` axis was planned but unmeasured. `verify`
must never execute or attach to the target (ADR-0001), yet the axis is about
whether runtime instrumentation (debugger, Frida, V8 inspector) works. A full
multi-language guard library embedded in artifacts is a large undertaking.

## Decision

- Ship `direa_core::guard`: detection of debuggers/instrumentation via a
  non-zero `TracerPid`, process-map signatures (Frida agent/gadget, LLDB/GDB,
  hooking frameworks) and injected-library environment variables
  (`LD_PRELOAD`, `DYLD_INSERT_LIBRARIES`, `FRIDA`, `NODE_OPTIONS=--inspect`).
  A pure `classify` function makes it unit-testable.
- `verify --with-runtime` checks the **analysis environment**. When instrumentation
  is detected, the `runtime-instrumentation-exposure` axis is added (exposed);
  when nothing is detected the environment is *unknown*, so the axis is **skipped**,
  never scored as resisted (K-004).
- `verify` still never executes or attaches to the target.

## Consequences

### Positive
- The axis exists, is honest, and is reusable as an embeddable guard.
- No target execution, consistent with ADR-0001.

### Negative / risks
- It measures the environment, not the artifact; an attacker on a clean host is
  not detected. Embedding the guard into artifacts is future work.
- `[runtime] instrumentation_signatures` is not yet consumed by the guard
  (tracked as B-021).

## Ergonomics

Run `die-a-rea verify --original A --hardened B --with-runtime` in the environment
you care about. Evidence: `crates/direa-core/src/guard.rs` (unit tests),
`crates/direa-verify/src/runtime.rs` (unit tests); demo — with `FRIDA` set the
axis appears and the RRS drops.

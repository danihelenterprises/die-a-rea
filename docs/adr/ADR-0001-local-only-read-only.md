# ADR-0001 — Local-only, read-only analysis

- **Status:** Accepted
- **Date:** 2026-10-12
- **Deciders:** die-a-rea maintainers
- **Supersedes:** none

## Context

die-a-rea processes artifacts that may be sensitive (proprietary binaries,
bundles, secrets in strings). The reference adversary, REA, is itself documented
as local-only; it reads artifacts and never uploads them. A protection tool that
phones home or mutates the artifact would be both a privacy risk and a
correctness risk: the operator must be able to trust that hardening does not
change runtime behaviour beyond the intended transforms.

## Decision

- **No network egress, upload, or telemetry** from any die-a-rea component.
- **`verify` observes; it never executes or mutates the target.** Static probes
  read bytes only. The single external process `verify` may launch is the
  optional REA adversary — a separate tool the operator opts into with
  `--with-rea`.
- `build` writes only to the chosen output directory; it never edits the source
  artifact in place.

## Consequences

### Positive
- The tool is safe to run on closed-source or regulated artifacts.
- `verify` results are reproducible and side-effect free.
- No data-protection or exfiltration surface.

### Negative / risks
- Runtime instrumentation (detecting a debugger while the app runs) cannot be
  measured by `verify` alone; it needs the planned guard library (B-007) and a
  separate harness.

## Ergonomics

Easy to follow: the core crates do no I/O beyond reading the target and writing
the output directory. The temptation to add "just a crash-report ping" is the
failure mode; reject it or supersede this ADR.

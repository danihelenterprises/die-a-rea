# ADR-0004 — Measure resistance with RRS; never claim unbreakability

- **Status:** Accepted
- **Date:** 2026-10-12
- **Deciders:** die-a-rea maintainers
- **Supersedes:** none

## Context

Every obfuscator markets "unbreakable". No local artifact is
un-reverse-engineerable. A tool that promises absolute protection is either
lying or setting its users up to make bad decisions. die-a-rea differentiates by
measuring the improvement instead of asserting it.

## Decision

- Every protection has a **measured** counterpart: the Reverse-engineering
  Resistance Score (RRS), computed in `direa-core::rrs` and produced by
  `direa-verify`.
- `build` writes a `die-a-rea.build.json` sidecar recording what was injected, so
  verification can distinguish decoy content from real content
  (`crates/direa-core/src/buildmeta.rs`).
- Documentation must state the limits plainly (see `README.md` "Scope and
  honesty", `docs/THREAT-MODEL.md`).
- Axis weights are part of the measurement contract; changing them to inflate a
  score is forbidden (see ADR-0005).

## Consequences

### Positive
- Users can gate CI on resistance and see regressions.
- Marketing claims are bounded by reproducible numbers.

### Negative / risks
- The score is only as good as its probes; a poor probe can mislead. Probes must
  be transparent and testable.

## Ergonomics

Every new capability is expected to ship with the probe that proves it. Trap:
adding a transform without a corresponding measurement, leaving RRS unchanged.

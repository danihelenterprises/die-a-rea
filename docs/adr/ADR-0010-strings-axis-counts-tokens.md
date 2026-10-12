# ADR-0010 — The `strings` axis counts identifier/literal tokens

- **Status:** Accepted
- **Date:** 2026-10-12
- **Deciders:** die-a-rea maintainers
- **Supersedes:** none (extends ADR-0005)

## Context

The `strings` probe counted contiguous printable runs. K-013 showed this
collapses a minified, single-line bundle into one run, making the string axis
useless for JavaScript. The build's injected-run accounting used the same
printable-run logic, so both had to change together.

## Decision

- The `strings` probe and the build's `injected_string_runs` accounting both
  count **identifier/literal tokens**: maximal runs of `[A-Za-z0-9_$]` at least
  6 bytes long, via `direa_core::scan::count_tokens`.
- Native section-name scrubbing fills with a **non-token** byte (space) so a
  scrubbed name is not itself counted as a token.
- The axis stays **signal-to-noise** (ADR-0005): `real / (real + injected)` when
  build metadata is present.

## Consequences

### Positive
- Robust to minification; comparable across text and binary artifacts.
- One shared definition (`direa_core::scan`) prevents probe/accounting drift.

### Negative / risks
- A static byte probe still cannot measure string *encoding*: base64/hex blobs
  are counted as tokens, so encoding-based obfuscation may not lower exposure
  (K-015). Encoder-aware discounting is tracked as B-018.

## Ergonomics

Use `direa_core::scan::count_tokens` everywhere; never reintroduce a second
definition of "string". Evidence: `crates/direa-core/src/scan.rs` (tests),
`crates/direa-verify/src/probes.rs`, `crates/direa-cli/src/build.rs`.

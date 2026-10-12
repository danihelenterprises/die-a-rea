# ADR-0005 — Signal-to-noise strings and skip inapplicable axes

- **Status:** Superseded by ADR-0011
- **Date:** 2026-10-12
- **Deciders:** die-a-rea maintainers
- **Supersedes:** none

## Context

Naive resistance metrics are easy to game or misinterpret:

1. Counting recovered strings punishes decoys: injecting 500 fake strings raises
   the raw count, making a *better*-protected artifact look worse.
2. Scoring an axis a target has no signal for (e.g. x86 prologues in a JS file,
   or symbols in a stripped file) silently awards full resistance for an
   inapplicable measurement.

## Decision

- **Strings are signal-to-noise.** When build metadata is present, string
  exposure is `real / (real + injected)`, using
  `InjectedStats::injected_string_runs` written by `build`.
- **Inapplicable axes are skipped, not scored.** A probe whose baseline reading
  is zero (or that errors) is omitted with a note, never counted as resisted.
- These rules live in `crates/direa-verify/src/harness.rs` (`measure`).

## Consequences

### Positive
- Decoys correctly improve the score.
- The score reflects only what was actually measured; coverage is visible in
  notes.

### Negative / risks
- Reports vary in which axes appear, so a raw score is not comparable across
  unrelated targets without reading the notes.

## Ergonomics

When adding a probe, decide its `ProbeKind` and whether baseline-zero means
"skip". Evidence: `direa-verify/src/harness.rs`; test `end_to_end_static`;
`buildmeta.rs` field `injected_string_runs`.

# ADR-0003 — Deterministic, seeded randomization

- **Status:** Accepted
- **Date:** 2026-10-12
- **Deciders:** die-a-rea maintainers
- **Supersedes:** none

## Context

Randomization (decoys, shuffling, depth bombs) is a core protection layer, but
nondeterministic output makes builds irreproducible, breaks binary-diff review,
and makes verification flaky. Operators also need to be able to reproduce a
specific hardened artifact for support and audits.

## Decision

All randomness flows through `direa_core::Rng` (SplitMix64).

- `[randomization] seed = 0` derives a stable seed from project name + version +
  target path (`derive_seed`).
- A nonzero seed (manifest or `--seed`) is fully reproducible.
- No component may use unseeded randomness.

## Consequences

### Positive
- Reproducible builds; the same inputs give the same hardened output.
- Per-release seeds give each version a fresh layout while staying debuggable.
- Verification is deterministic.

### Negative / risks
- Changing the seed derivation or drawing order changes all outputs; that is a
  breaking change and must be treated as one.

## Ergonomics

Easy: use `Rng`, never `rand::thread_rng`. Evidence: `random.rs` tests
`deterministic_for_same_seed` and `derived_seed_is_stable`; also lesson K-001.

# ADR-0011 — String exposure measures verbatim survival

- **Status:** Accepted
- **Date:** 2026-10-12
- **Deciders:** die-a-rea maintainers
- **Supersedes:** ADR-0005 (string formula); the skip-inapplicable-axes rule is retained

## Context

The `strings` axis was a count ratio, then a token count (ADR-0010). Both fail to
measure what matters. Empirical evidence:

- `javascript-obfuscator` encodes strings **and** introduces many new
  identifiers, so a count ratio stays at/above 1.0 even though the original
  literals are gone (K-015).
- Decoys add tokens, inflating a count ratio the wrong way.

A count cannot distinguish "the original strings are hidden" from "there are more
strings now".

## Decision

The `strings` axis measures **verbatim survival**:

- Take the set of distinct identifier/literal tokens (>= 6 bytes) in the
  **baseline** artifact and in the **hardened** artifact.
- `hardened_exposure = |baseline ∩ hardened| / |baseline|`; baseline exposure is
  `1.0` (or the axis is skipped when the baseline has no tokens).
- Token extraction is `direa_core::scan::token_runs`; a token is a maximal run of
  `[A-Za-z0-9_$]`.

Encoding needs no heuristic: an encoded literal is simply not one of the
baseline's tokens, so survival drops. Dilution by decoys is scored separately by
the analysis-noise axis. `InjectedStats::injected_string_runs` is retained as
informational metadata but no longer feeds the string axis.

The rule from ADR-0005 that **inapplicable axes are skipped, not scored** is
retained.

## Consequences

### Positive
- Measures whether the *original* recoverable signal survives, across text and
  binary artifacts, with no format- or encoder-specific heuristics.
- Robust: adding decoys or mangling identifiers cannot inflate it.

### Negative / risks
- Common keywords (`function`, `return`, `length`, `module`) survive obfuscation
  and are counted, so exposure has a floor above zero for JS.

## Ergonomics

Use `scan::token_runs` on both artifacts and compare sets; never revert to a
count ratio for this axis. Evidence: `crates/direa-verify/src/harness.rs`
(`token_set`, strings branch); demos — obfuscated JS 5/9 tokens survive;
`native-post` ELF 0.32.

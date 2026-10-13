# ADR-0020 — The canary detector normalizes before matching

- **Status:** Accepted
- **Date:** 2026-10-12
- **Deciders:** die-a-rea maintainers
- **Supersedes:** none

## Context

The built-in prompt-injection detector (`canary::detect_injection`) matched
markers with a plain lowercase `contains`. ADR-0016 already flagged this as
"regex-based and evadable". A red-team corpus (B-041) confirms it: every sample
that hid a marker behind a zero-width character, a whitespace run, a Unicode
homoglyph, or an HTML entity slipped past a naive substring match. Detection is
the regression test for the content we emit (K-025/K-026), so a trivially
evadable detector gives a false sense of coverage.

## Decision

- **Normalize before matching.** Detection runs on a canonical form produced by
  `canary::normalize_for_detection`: HTML entities decoded (numeric decimal/hex
  and a small named set), invisible formatting characters dropped, the common
  homoglyph set mapped to ASCII, case folded, and whitespace runs collapsed to a
  single space. Markers are normalized the same way.
- **Red-team the detector.** The gauntlet includes a fixed `EVASION_CORPUS` of
  normalization-requiring samples and `passed` requires all of them to be caught,
  alongside the existing adversarial corpus. A unit test asserts the corpus
  defeats a naive lowercase `contains`, so the red-team cannot rot into
  green-washing.
- **This is a floor, not a ceiling.** Normalization raises the bar against cheap
  evasions; determined obfuscation is still handled by the external detector hook
  (K-029/ADR-0006). Detection is best-effort and must never be advertised as
  robust (ADR-0004).

## Consequences

### Positive
- Cheap, common evasions no longer defeat the detector the gauntlet relies on.
- The evasion corpus makes detector regressions visible in CI
  (`die-a-rea canary-selftest`).
- Normalization is a single, testable pure function, so honest payloads stay
  clean (asserted by `normalization_does_not_flag_benign_text`).

### Negative / risks
- Normalization can only cover the transformations it knows; it is explicitly
  incomplete and must not be read as a guarantee.
- Homoglyph folding adds a small false-positive surface; the markers are
  multi-word, so the practical risk is low and covered by tests.

## Ergonomics

Contributors extending the detector add a marker to `INJECTION_MARKERS` and, for
each new evasion style, a sample to `EVASION_CORPUS`; `canary-selftest` and the
`evasion_corpus_defeats_naive_matching` test fail if the corpus stops being a
real challenge. Evidence: `crates/direa-core/src/canary.rs`
(`normalize_for_detection`, `EVASION_CORPUS`, `gauntlet_with`);
`crates/direa-cli/src/main.rs` (`cmd_canary_selftest`). Threat model unchanged
(`docs/THREAT-MODEL.md`); responsible-use boundaries unchanged (`docs/ETHICS.md`).

# ADR-0016 — Deceptive decoys are opt-in, bounded, and gauntlet-validated

- **Status:** Accepted
- **Date:** 2026-10-12
- **Deciders:** die-a-rea maintainers
- **Supersedes:** none

## Context

The agent layer can include *deceptive* content (fake analysis notes,
operator-supplied payloads) that misleads an LLM analyst. This is dual-use and
must never be the default, unbounded, or a way to hide malicious behaviour.
B-008 delivered the content generator; B-009 delivered the detector gauntlet.

## Decision

- **Opt-in only.** `agent_decoys` is `false` in `ResolvedPolicy::default` and
  requires an explicit manifest override; `custom_payloads` is empty by default.
- **Bounded.** Decoy analysis text is capped at `canary::MAX_DECOY_LINES` (200).
  `custom_payloads` is capped at `MAX_CUSTOM_PAYLOADS` (32) entries, each
  truncated to `MAX_CUSTOM_PAYLOAD_LEN` (4096) and sanitized (`*/` escaped).
- **Validated.** The detector gauntlet (B-009) must catch a fixed adversarial
  corpus, and our default gate/decoy payloads must not be flagged as injections.
- **Never for concealment.** Decoys must not be used to hide malicious behaviour
  (`docs/ETHICS.md`, ADR-0006).

## Consequences

### Positive
- Deception cannot be enabled accidentally or grown without bound.
- The gauntlet gives a regression test for both detection and payload cleanliness.

### Negative / risks
- The detector is regex-based and evadable; this bounds blast radius, not
  guarantees stealth. The arms race continues (K-015/K-016 spirit).

## Ergonomics

Turn it on deliberately: `[policy.overrides] agent_decoys = true`, and keep
`custom_payloads` short. Evidence: `crates/direa-cli/src/build.rs` (caps,
`custom_payloads_are_bounded_and_sanitized`), `crates/direa-core/src/canary.rs`
(`MAX_DECOY_LINES`, `gauntlet`).

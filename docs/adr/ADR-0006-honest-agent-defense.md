# ADR-0006 — Honest-by-default agent defense; deception opt-in

- **Status:** Accepted
- **Date:** 2026-10-12
- **Deciders:** die-a-rea maintainers
- **Supersedes:** none

## Context

The agent layer (making an LLM analysis agent's job harder) is the
REA-specific value. But prompt-injection-style deception is dual-use and
detectable; published research shows agent defenses can be trained and evaded.
Shipping deception by default would also risk misleading defenders about what
software does, which conflicts with the project's purpose.

## Decision

- The **default** agent defense is honest: an authorization notice surfaced as a
  real string literal, plus traceable canaries.
  `ResolvedPolicy::default` sets `agent_stop_notice = true`, `agent_canaries =
  false`, `agent_decoys = false`; profiles may enable canaries, but decoys stay
  gated.
- **Deceptive payloads (`agent_decoys`, `custom_payloads`) are opt-in** and
  experimental, and may not be enabled by default until validated (B-009/B-013).
- `docs/ETHICS.md` governs use: protect only software you own or are authorized
  to protect; never hide malicious behaviour.

## Consequences

### Positive
- The tool remains defensible and honest; AV/analyst trust is preserved.
- A clear boundary for contributors.

### Negative / risks
- Honest notices are easier for a determined agent to ignore than deception;
  the protective value is lower by default.

## Ergonomics

Default path is safe: you must deliberately set `agent_decoys = true` (and write
an ADR for any default change). Evidence: `crates/direa-core/src/policy.rs`,
`crates/direa-core/src/manifest.rs`; `docs/TARPITS.md`, `docs/ETHICS.md`.

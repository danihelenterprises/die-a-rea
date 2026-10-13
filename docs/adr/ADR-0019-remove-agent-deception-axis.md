# ADR-0019 — Remove the unmeasured agent-deception axis

- **Status:** Accepted
- **Date:** 2026-10-12
- **Deciders:** die-a-rea maintainers
- **Supersedes:** none

## Context

`Axis::AgentDeception` (weight 2.0) was declared but **never produced by any
probe**: judging whether an LLM was *deceived* requires an LLM, which `verify`
does not run. A declared axis with the largest weight that no measurement ever
populates is misleading — it suggests a capability the tool does not have.

## Decision

Remove `Axis::AgentDeception` from the RRS model. Agent-layer efficacy is instead
validated by:

- the detector **gauntlet** (`canary::gauntlet`, `die-a-rea canary-selftest`), and
- **canary provenance** (traceable tokens in build metadata).

An agent-layer RRS axis may be reintroduced only with a probe that actually
measures it (e.g. a configured LLM adversary) and a superseding ADR.

## Consequences

### Positive
- The RRS no longer advertises an unmeasured axis.
- Smaller, honest contract.

### Negative / risks
- The REA-specific layer has no RRS axis; its evidence is the gauntlet and
  canaries. This is documented rather than implied.

## Ergonomics

Nothing to configure. Evidence: `crates/direa-core/src/rrs.rs` (axis removed,
test updated); `docs/RRS.md` (row removed); `canary::gauntlet` (B-009).

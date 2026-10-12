# ADR-0017 — The control-flow axis requires an external decompiler

- **Status:** Accepted
- **Date:** 2026-10-12
- **Deciders:** die-a-rea maintainers
- **Supersedes:** none (extends ADR-0005/0011)

## Context

The control-flow axis was measured by counting x86 function-prologue byte
signatures — a proxy that is easy to fool and meaningless on non-x86 artifacts.
A real measurement needs a decompiler or disassembler, which die-a-rea does not
bundle (ADR-0007).

## Decision

- The control-flow axis is **not** a default probe. It requires an external
  command configured with `verify --decompiler "<cmd>"` (`{artifact}`
  substituted).
- `ExternalControlFlowProbe` runs the command and reads an integer count of
  recoverable functions from its stdout; exposure is the usual baseline ratio.
- When no decompiler is configured, the axis is **skipped**, never fabricated
  (K-004).
- `tools/controlflow-count` is a reference command (nm/llvm-nm/objdump based).

## Consequences

### Positive
- The axis reflects real tooling instead of a brittle byte signature.
- Consistent with ADR-0005/0011: measure only what can be measured.

### Negative / risks
- Native targets lose the axis unless the operator wires a decompiler.
- `tools/controlflow-count` is a disassembler-based count, not a full decompiler;
  it is honest about that.

## Ergonomics

```bash
die-a-rea verify --original app --hardened dist/app \
    --decompiler "tools/controlflow-count {artifact}"
```

Evidence: `crates/direa-verify/src/probes.rs` (`ExternalControlFlowProbe`, test
`external_control_flow_reads_a_count`); `crates/direa-verify/src/harness.rs`
(probe added only when configured); lesson K-030.

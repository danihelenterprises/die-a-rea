# ADR-0007 — MIT core; copyleft/proprietary tools behind the subprocess boundary

- **Status:** Accepted
- **Date:** 2026-10-12
- **Deciders:** die-a-rea maintainers
- **Supersedes:** none

## Context

The best protection tooling spans licenses: MIT/Apache (Pluto, O-MVLL,
ConfuserEx), BSD (garble, javascript-obfuscator), and GPL (ProGuard), plus
commercial tools (VMProtect, Themida, DexGuard). die-a-rea itself is MIT. Linking
copyleft code would make MIT distribution impossible; bundling commercial tools
would be a licensing violation.

## Decision

- The die-a-rea core is **MIT** (`LICENSE`, `Cargo.toml`).
- Copyleft and proprietary toolchains are **never linked**. They run as adapter
  subprocesses (ADR-0002).
- Commercial tools are **bring-your-own**; die-a-rea grants no license to them.
- Third-party license compatibility is tracked in `docs/PIGGYBACK.md`.

## Consequences

### Positive
- Clean MIT distribution.
- Users bring their own licensed toolchains, mirroring REA's provider model.

### Negative / risks
- Adapters depend on tools the host must install; capability gaps must be
  reported (the planner already reports missing adapters).

## Ergonomics

Rule of thumb: if it is not MIT/Apache/BSD, it lives behind `SubprocessAdapter`.
Evidence: `Cargo.toml` (`license = "MIT"`), `docs/PIGGYBACK.md`, `adapter.rs`.

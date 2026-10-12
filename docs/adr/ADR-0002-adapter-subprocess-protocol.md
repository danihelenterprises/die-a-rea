# ADR-0002 — Adapters are subprocesses over a JSON protocol

- **Status:** Accepted
- **Date:** 2026-10-12
- **Deciders:** die-a-rea maintainers
- **Supersedes:** none

## Context

Ecosystem-specific hardening needs toolchains in many languages (LLVM/Clang,
Go, Node, .NET, JVM/Android). Some are copyleft, some proprietary. The core must
stay language-agnostic, buildable without those toolchains, and license-clean.

## Decision

Adapters are **separate programs** that speak a small JSON request/response
protocol over stdio:

```
die-a-rea (host)  --JSON request-->  adapter (stdin)
die-a-rea (host)  <--JSON response-- adapter (stdout)
```

- The host runs an adapter as a subprocess (`crates/direa-core/src/adapter.rs`,
  `SubprocessAdapter`).
- Each request carries a `protocol` version
  (`direa_core::adapter::proto_version`; currently `1`) so adapters can negotiate.
- Operations: `capabilities`, `plan`, `apply`, `verify`.
- The registry lives in `crates/direa-core/src/planner.rs` (`default_registry`).

## Consequences

### Positive
- The core has no toolchain-specific dependencies.
- Copyleft/proprietary tools run behind a process boundary and are never linked
  (see ADR-0007).
- A missing adapter degrades to a reported gap, not a build failure.

### Negative / risks
- IPC overhead and a versioning burden.
- The protocol is a public contract; changing it requires an ADR (see AGENTS
  frozen areas).

## Ergonomics

Adding an adapter is mechanical: implement the JSON protocol, register the
command in the planner, add a test that round-trips a request. Trap: forgetting
to bump `proto_version` on an incompatible change.

# BACKLOG — open-problem registry

Canonical registry of every known hole. One row per item, stable id `B-NNN`,
never reused. Status ∈ {Open, In-progress, Decided, Deferred, Closed}.

Severity: **High** (blocks the core value), **Medium** (important, not blocking),
**Low** (hygiene).

Seeded from verified sources only: `README.md` (Repository status / Roadmap),
`docs/RRS.md` (`runtime-instrumentation-exposure` marked *planned*), the adapter
registry in `crates/direa-core/src/planner.rs`, and the absence of a git remote
(`git rev-parse` fails).

| ID | Item | Severity | Area/Milestone | Status | Notes |
|---|---|---|---|---|---|
| B-001 | Implement the LLVM adapter (C/C++/Rust/Swift) wrapping Pluto/O-MVLL passes | High | Adapters | Open | `planner.rs` registers `llvm` → `direa-llvm`, which does not exist yet. |
| B-002 | Implement the Go adapter (garble: identifiers, literals, strip build info) | High | Adapters | Open | REA has a built-in Go provider; stripping build info is the direct counter. |
| B-003 | Implement the JavaScript/Electron adapter (javascript-obfuscator + bytenode/asarmor) | High | Adapters | Open | Highest value: REA's JS path needs no native engine. |
| B-004 | Implement built-in `native-post` stripping (symbols/debug/build metadata) for ELF/Mach-O/PE | High | Adapters | Open | Registry lists `native-post` as `builtin`, but `build.rs` copies native artifacts unchanged. |
| B-005 | Implement the .NET adapter (ConfuserEx driver) | Medium | Adapters | Open | Registry lists `clr`; no implementation. |
| B-006 | Implement the Android/JVM adapter (R8/ProGuard config generation) | Medium | Adapters | Open | Registry lists `android`; Promises GPL tool stays subprocess-only (ADR-0007). |
| B-007 | Add a runtime guard library and wire the `runtime-instrumentation-exposure` RRS axis | High | Runtime | Open | `docs/RRS.md` marks the axis *planned*; no guard code exists. |
| B-008 | Implement the `direa-canary` adapter (detector-hardened agent payloads) | Medium | Agent | Open | README Roadmap; `agent_decoys` today only injects comments/strings. |
| B-009 | Add the detector-hardening gauntlet (NPS-style prompt-injection detector vs our canaries) | Medium | Agent | Open | README Roadmap; depends on B-008. |
| B-010 | Configure a git remote; repository history begins at the memory-install snapshot | High | Ops | Open | `git rev-parse` failed before install; no remote exists. |
| B-011 | Add `CONTRIBUTING.md` and `CHANGELOG.md` | Low | Docs | Open | Neither file exists. |
| B-012 | Declare MSRV (`rust-version`) in the workspace | Low | Build | Open | `Cargo.toml` has no `rust-version`; CI pins `stable`. |
| B-013 | Validate and bound deceptive `agent_decoys` payloads before any default use | Medium | Agent | Open | `docs/ETHICS.md` and ADR-0006 keep deception opt-in; depends on B-009. |
| B-014 | Strengthen the docs-check counter-duplication heuristic | Low | Tooling | Open | `tools/docs-check.sh` check 5 is best-effort (see STATE). |

## Closed / Decided

*(none yet)*

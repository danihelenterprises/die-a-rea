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
| B-002 | Implement the Go adapter (garble: identifiers, literals, strip build info) | High | Adapters | Closed | Built-in Go build-info/ID scrub in `native_post` (K-019); source-level identifier/literal obfuscation via `garble` remains bring-your-own. |
| B-003 | Implement the JavaScript/Electron adapter (javascript-obfuscator + bytenode/asarmor) | High | Adapters | Closed | Adapter implemented, wired, tested (ADR-0009). Its `string-exposure` clause passes: obfuscated 5/9 baseline tokens survive vs 9/9 for the built-in path (ADR-0011). |
| B-004 | Implement built-in `native-post` stripping (symbols/debug/build metadata) for ELF/Mach-O/PE | High | Adapters | Closed | In-process `native_post.rs` (ELF/PE/Mach-O); wired via `harden_native`. Mach-O DWARF/fat + PE resources remain follow-ups. |
| B-005 | Implement the .NET adapter (ConfuserEx driver) | Medium | Adapters | Open | Registry lists `clr`; no implementation. |
| B-006 | Implement the Android/JVM adapter (R8/ProGuard config generation) | Medium | Adapters | Open | Registry lists `android`; Promises GPL tool stays subprocess-only (ADR-0007). |
| B-007 | Add a runtime guard library and wire the `runtime-instrumentation-exposure` RRS axis | High | Runtime | Closed | `direa_core::guard` + `verify --with-runtime` (ADR-0014, K-020); detects debugger/Frida/inspector from the environment. |
| B-021 | Honor `[runtime] instrumentation_signatures` in the guard | Low | Runtime | Closed | `guard::detect_with` + `verify` reads the manifest list (empty = built-in defaults). |
| B-022 | Docker toolchain (runtime + dev images) with robust error handling | Medium | Tooling | Closed | `docker/Dockerfile` (build/runtime/dev), `entrypoint.sh` (fail-fast), `compose.yml` (ADR-0015, K-021). |
| B-008 | Implement the `direa-canary` adapter (detector-hardened agent payloads) | Medium | Agent | Closed | `direa_core::canary` (gate/canary/decoy) wired into `build`; detector added under B-009 (K-025). |
| B-009 | Add the detector-hardening gauntlet (NPS-style prompt-injection detector vs our canaries) | Medium | Agent | Closed | `canary::gauntlet` + `die-a-rea canary-selftest` in CI (K-026). |
| B-010 | Configure a git remote; repository history begins at the memory-install snapshot | High | Ops | Closed | `origin` = `git@github-die-a-rea:danihelenterprises/die-a-rea.git`; `main` pushed. |
| B-011 | Add `CONTRIBUTING.md` and `CHANGELOG.md` | Low | Docs | Closed | Both added and indexed in `docs/README.md`. |
| B-012 | Declare MSRV (`rust-version`) in the workspace | Low | Build | Closed | `rust-version = "1.79"` in `[workspace.package]`, inherited by every crate; toolchain recorded as K-022. |
| B-013 | Validate and bound deceptive `agent_decoys` payloads before any default use | Medium | Agent | Open | `docs/ETHICS.md` and ADR-0006 keep deception opt-in; depends on B-009. |
| B-014 | Strengthen the docs-check counter-duplication heuristic | Low | Tooling | Closed | Fatal by default, broader labels, and `--self-test` (wired into CI). |
| B-015 | Extend `native-post` to Mach-O DWARF sections and fat containers | Medium | Adapters | Closed | `strip_fat` iterates slices; `__DWARF` section data zeroed and names scrubbed; `detect` recognises fat magics (K-024). |
| B-016 | Extend `native-post` to PE resources and the rich header | Low | Adapters | Closed | Rich header fingerprint stripped; resources intentionally preserved as runtime-needed (K-023). |
| B-017 | Make the `strings` probe token/literal-aware so minified JS scores correctly | High | RRS | Closed | Done: `direa_core::scan::count_tokens` (ADR-0010). A minified line no longer collapses to one sequence. |
| B-018 | Make the `strings` axis respond to string encoding | High | RRS | Closed | Solved as verbatim survival rather than base64 detection (ADR-0011, K-016): obfuscated JS now scores 0.56. |
| B-019 | Guard `build` against overwriting the source artifact or a foreign output directory | High | CLI | Closed | `guard_source_and_output` plus the non-empty-`--out` check; `--force` overrides (ADR-0012, K-017). |
| B-020 | Post-build health check so a broken hardened artifact fails the build | High | CLI | Closed | Always-on structural check plus opt-in `[verify]` command / `js_syntax` (ADR-0013, K-018). |

## Closed / Decided

- **B-010 (Closed, commit 3):** configured `origin` and pushed `main` to
  `danihelenterprises/die-a-rea` via a GitHub-over-443 deploy key.
- **B-004 (Closed, commit 4):** built-in `native-post` strips ELF/PE/Mach-O
  symbol/debug/build metadata in-process (ADR-0008). Follow-ups tracked as
  B-015 (Mach-O DWARF/fat) and B-016 (PE resources/rich header).
- **B-017 (Closed, commit 6):** `strings` probe now counts identifier/literal
  tokens via `direa_core::scan` (ADR-0010); minified files no longer collapse to
  a single sequence.
- **B-018 (Closed, commit 7):** `strings` axis redefined as verbatim survival
  (ADR-0011, K-016); base64/hex heuristics unnecessary. Closed **B-003** too —
  its `string-exposure` clause now passes.
- **B-019 (Closed, commit 8):** `build` refuses to overwrite the source artifact
  or a foreign output directory; `--force` overrides (ADR-0012, K-017).
- **B-020 (Closed, commit 9):** `build` health-checks the hardened artifact and
  fails on breakage — non-empty + native magic, plus opt-in `[verify]` command and
  `js_syntax` (ADR-0013, K-018).
- **B-002 (Closed, commit 10):** built-in Go build-info/ID scrub in `native_post`
  (K-019). `garble` remains bring-your-own for identifier/literal obfuscation.
- **B-007 (Closed, commit 11):** `direa_core::guard` detects debugger/Frida/
  inspector from the environment; `verify --with-runtime` adds the
  runtime-instrumentation axis (ADR-0014, K-020). Opened B-021 (manifest
  signatures not yet consumed).
- **B-022 (Closed, commit 12):** Docker toolchain — runtime/dev images plus a
  fail-fast entrypoint and compose file (ADR-0015, K-021).
- **B-012 (Closed, commit 13):** declared MSRV 1.79 across the workspace
  (K-022).
- **B-011 (Closed, commit 14):** added `CONTRIBUTING.md` and `CHANGELOG.md`.
- **B-021 (Closed, commit 15):** `verify` now feeds `[runtime]
  instrumentation_signatures` into `guard::detect_with`.
- **B-014 (Closed, commit 16):** docs-check counter heuristic is fatal, broader,
  and has a `--self-test` wired into CI.
- **B-016 (Closed, commit 17):** `native-post` strips the PE Rich header; PE
  resources are preserved deliberately (K-023).
- **B-015 (Closed, commit 18):** `native-post` handles Mach-O fat containers and
  `__DWARF` segments (K-024).
- **B-008 (Closed, commit 19):** `direa_core::canary` provides the honest gate,
  canary tokens and bounded decoy text; `build` consumes them (K-025).
- **B-009 (Closed, commit 20):** detector gauntlet (`canary::gauntlet`,
  `canary-selftest`) wired into CI (K-026).

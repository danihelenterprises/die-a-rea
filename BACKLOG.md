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
| B-001 | Implement the LLVM adapter (C/C++/Rust/Swift) wrapping Pluto/O-MVLL passes | High | Adapters | Closed | Generic external-adapter mechanism + `adapters/llvm/direa-llvm` (compile-time hook via `DIREA_LLVM_BUILD`); toolchain bring-your-own (K-027). |
| B-002 | Implement the Go adapter (garble: identifiers, literals, strip build info) | High | Adapters | Closed | Built-in Go build-info/ID scrub in `native_post` (K-019); source-level identifier/literal obfuscation via `garble` remains bring-your-own. |
| B-003 | Implement the JavaScript/Electron adapter (javascript-obfuscator + bytenode/asarmor) | High | Adapters | Closed | Adapter implemented, wired, tested (ADR-0009). Its `string-exposure` clause passes: obfuscated 5/9 baseline tokens survive vs 9/9 for the built-in path (ADR-0011). |
| B-004 | Implement built-in `native-post` stripping (symbols/debug/build metadata) for ELF/Mach-O/PE | High | Adapters | Closed | In-process `native_post.rs` (ELF/PE/Mach-O); wired via `harden_native`. Mach-O DWARF/fat + PE resources remain follow-ups. |
| B-005 | Implement the .NET adapter (ConfuserEx driver) | Medium | Adapters | Closed | `adapters/clr/direa-clr` runs `DIREA_CLR_COMMAND` (ConfuserEx); toolchain bring-your-own (K-027). |
| B-006 | Implement the Android/JVM adapter (R8/ProGuard config generation) | Medium | Adapters | Closed | `adapters/jvm/direa-jvm` runs `DIREA_JVM_COMMAND` (R8/ProGuard) as a subprocess (ADR-0007); bring-your-own. |
| B-007 | Add a runtime guard library and wire the `runtime-instrumentation-exposure` RRS axis | High | Runtime | Closed | `direa_core::guard` + `verify --with-runtime` (ADR-0014, K-020); detects debugger/Frida/inspector from the environment. |
| B-021 | Honor `[runtime] instrumentation_signatures` in the guard | Low | Runtime | Closed | `guard::detect_with` + `verify` reads the manifest list (empty = built-in defaults). |
| B-022 | Docker toolchain (runtime + dev images) with robust error handling | Medium | Tooling | Closed | `docker/Dockerfile` (build/runtime/dev), `entrypoint.sh` (fail-fast), `compose.yml` (ADR-0015, K-021). |
| B-023 | Embed the runtime guard into artifacts (not just the environment) | Medium | Runtime | Closed | Behavior-safe JS/Electron guard injected when `anti_instrumentation` is set (K-028); native embedding tracked as B-027. |
| B-024 | Pluggable/external detector behind the gauntlet; broaden the corpus | Medium | Agent | Closed | `canary::gauntlet_with` + `canary-selftest --detector`; corpus broadened to 7 styles (K-029). |
| B-025 | Real-toolchain CI lanes for the adapters (Pluto/ConfuserEx/R8) as optional jobs | Low | Tooling | Closed | `adapter-js` (real) plus optional `adapter-clr`/`adapter-jvm`/`adapter-llvm` jobs; documented in `docs/ADAPTERS.md`. |
| B-026 | Real control-flow probe (decompiler-based) instead of the x86-prologue proxy | Medium | RRS | Closed | `ExternalControlFlowProbe` via `verify --decompiler`; proxy removed; axis skipped when unset (ADR-0017, K-030). |
| B-027 | Embed the runtime guard into native artifacts | Low | Runtime | Closed | Linkable C guard at `runtime/guard/` (ADR-0018, K-031); link-time, not injected. |
| B-028 | CLI verbosity, phase timing, and a transform trace | Medium | Tooling | Closed | `-v`/`-vv`/`-q` with `ui` module and per-phase timing (K-032). |
| B-029 | Stable error codes and a JSON error envelope | Medium | Tooling | Closed | `DireaError::code()`, coded build refusals, `--error-format json` (K-033). |
| B-030 | `die-a-rea doctor` readiness command | Medium | Tooling | Closed | `die-a-rea doctor [--json]`; adapter id `android`→`jvm` aligned (K-034). |
| B-031 | `die-a-rea explain <id>` for K/ADR/B entries | Low | Tooling | Closed | `die-a-rea explain <id> [--root DIR] [--json]` (K-035). |
| B-032 | Toolchain-free post for WASM and Java `.class` | Medium | Adapters | Closed | `wasm-post` (custom sections) + `java-post` (debug attributes) (K-036). |
| B-033 | GitHub Action for one-line integration | Medium | Tooling | Closed | Composite `action.yml` installing the CLI and running `args`. |
| B-034 | Config auto-discovery and smarter `init` | Medium | Tooling | Closed | `direa_core::discover` used by `init` (K-037). |
| B-035 | Publish crates + release binaries + optional C ABI | Medium | Tooling | Closed | crates.io metadata + dep versions, `release.yml`, `direa-ffi` cdylib (K-038). |
| B-036 | Cross-platform guard detection (Windows/macOS) | Medium | Runtime | Closed | Rust + C guard use `IsDebuggerPresent`/`P_TRACED` (K-039). |
| B-037 | SARIF and JUnit output for CI | Low | Tooling | Closed | `verify --format sarif|junit` (K-040). |
| B-038 | End-to-end eval harness + benchmark corpus + RRS regression gate | High | RRS | Closed | `bench` corpus + `--check` in CI, `tests/e2e.rs` (K-041). |
| B-039 | Remove or measure the unmeasured `agent-deception` RRS axis | Low | RRS | Closed | Axis removed; agent layer validated by the gauntlet (ADR-0019, K-042). |
| B-040 | Grow the bench corpus to every built-in format | Medium | RRS | Closed | JS file/tree, WASM, Java, ELF fixtures + generator (K-043). |
| B-008 | Implement the `direa-canary` adapter (detector-hardened agent payloads) | Medium | Agent | Closed | `direa_core::canary` (gate/canary/decoy) wired into `build`; detector added under B-009 (K-025). |
| B-009 | Add the detector-hardening gauntlet (NPS-style prompt-injection detector vs our canaries) | Medium | Agent | Closed | `canary::gauntlet` + `die-a-rea canary-selftest` in CI (K-026). |
| B-010 | Configure a git remote; repository history begins at the memory-install snapshot | High | Ops | Closed | `origin` = `git@github-die-a-rea:danihelenterprises/die-a-rea.git`; `main` pushed. |
| B-011 | Add `CONTRIBUTING.md` and `CHANGELOG.md` | Low | Docs | Closed | Both added and indexed in `docs/README.md`. |
| B-012 | Declare MSRV (`rust-version`) in the workspace | Low | Build | Closed | `rust-version = "1.79"` in `[workspace.package]`, inherited by every crate; toolchain recorded as K-022. |
| B-013 | Validate and bound deceptive `agent_decoys` payloads before any default use | Medium | Agent | Closed | Opt-in, capped, sanitized, gauntlet-validated (ADR-0016). |
| B-014 | Strengthen the docs-check counter-duplication heuristic | Low | Tooling | Closed | Fatal by default, broader labels, and `--self-test` (wired into CI). |
| B-015 | Extend `native-post` to Mach-O DWARF sections and fat containers | Medium | Adapters | Closed | `strip_fat` iterates slices; `__DWARF` section data zeroed and names scrubbed; `detect` recognises fat magics (K-024). |
| B-016 | Extend `native-post` to PE resources and the rich header | Low | Adapters | Closed | Rich header fingerprint stripped; resources intentionally preserved as runtime-needed (K-023). |
| B-017 | Make the `strings` probe token/literal-aware so minified JS scores correctly | High | RRS | Closed | Done: `direa_core::scan::count_tokens` (ADR-0010). A minified line no longer collapses to one sequence. |
| B-018 | Make the `strings` axis respond to string encoding | High | RRS | Closed | Solved as verbatim survival rather than base64 detection (ADR-0011, K-016): obfuscated JS now scores 0.56. |
| B-019 | Guard `build` against overwriting the source artifact or a foreign output directory | High | CLI | Closed | `guard_source_and_output` plus the non-empty-`--out` check; `--force` overrides (ADR-0012, K-017). |
| B-020 | Post-build health check so a broken hardened artifact fails the build | High | CLI | Closed | Always-on structural check plus opt-in `[verify]` command / `js_syntax` (ADR-0013, K-018). |
| B-041 | Red-team the canary detector against cheap evasions and harden it | Medium | Agent | Closed | Detector normalizes (entities/homoglyphs/invisibles/whitespace); `EVASION_CORPUS` + gauntlet gate + naive-matcher test (ADR-0020, K-044). |
| B-042 | Built-in hardening for Android APK (DEX/resources), or an explicit toolchain-only stance | Medium | Adapters | Open | Detection + verification only today; `jvm`/R8 is the sole path. Per-ecosystem gap audit 2026-10-12. |
| B-043 | Fix single-file Electron `.asar` handling (binary archive read as UTF-8) | High | Adapters | Closed | Fixed: a `.asar` file is copied verbatim (opaque archive) and deferred to an external adapter; no more UTF-8 error (K-049). Real built-in transforms tracked as B-055. |
| B-055 | Built-in asar unpack/harden/repack (beyond copy + adapter) | Medium | Adapters | Closed | `direa_core::asar` rewrites embedded JS (source-map/comment strip, notice, canary, guard) and repacks the archive with fresh offsets, dropping stale integrity; unparseable archives fall back to copy (K-056). Verified with `asar extract`. |
| B-044 | Bundle reference adapters for Go (`garble`) and WASM (`wasm-opt`/`wasm-mutate`) | Medium | Adapters | Closed | `adapters/go/direa-go` (DIREA_GO_COMMAND) and `adapters/wasm/direa-wasm` (DIREA_WASM_COMMAND) ship, are documented, have optional CI lanes, and were verified end-to-end in a container (K-053, K-054). |
| B-057 | Capture wrapped-tool output in the existing clr/jvm/llvm adapters | Low | Adapters | Open | They use `execSync(..., {stdio:'inherit'})`, so tool stdout corrupts the JSON response and errors are hidden; the new go/wasm adapters capture instead. |
| B-045 | Add a JVM `.class`/`.jar` external adapter lane | Medium | Adapters | Closed | Added `TargetKind::JavaJar` (detected by `.jar`), registered `jvm` for `JavaClass`/`JavaJar`/`AndroidApk`, and copy jars for the adapter (K-055). Built-in jar repack tracked as B-058. |
| B-058 | Built-in jar unpack/harden/repack (beyond copy + adapter) | Medium | Adapters | Open | B-045 routes `.jar` to the jvm adapter but copies it; a real fix unzips, runs java-post on each `.class`, and repacks. |
| B-046 | Extend guard embedding and canary/decoy/tarpit parity beyond JS | Medium | Runtime | Open | Canaries/notice/decoys are embedded only for JS; tarpits skip WASM/Java/APK; the native guard is link-time only. |
| B-047 | Add managed/bytecode/WASM RRS probes (Java, DEX, WASM names, .NET) | Medium | RRS | Closed | `debug-info` is now format-aware: native DWARF/PDB, JVM debug attributes and WASM custom sections, so `java-post`/`wasm-post` wins are measured (K-050). DEX debug still unmeasured (B-056). |
| B-056 | DEX debug-info measurement (needs a DEX parser) | Low | RRS | Open | APK hardening (B-042) and its measurement both need DEX parsing; DEX debug info has no stable byte marker. Surfaced by B-047. |
| B-048 | Extend the real REA adversary (`--with-rea`) beyond JS/asar targets | Medium | RRS | Open | `run_rea` uses `rea_js_target`, so only JS/Electron can gate on the adversary. |
| B-049 | Broaden `init` artifact discovery (Go/.NET/Android/C-C++) | Low | Tooling | Open | `discover` reads Cargo.toml/package.json/pom.xml only. |
| B-050 | Add bench fixtures for PE/Mach-O/Go/.NET/asar/APK | Medium | RRS | Closed | Synthetic native-pe/native-macho (Python) + electron-asar/android-apk, plus real Go/.NET built from toolchains when present; all gated in `expected.json` (go 41, dotnet 24 observed via a Go/.NET Docker container) (K-051, K-052). |
| B-051 | Teach `doctor` about `wasm-opt` and a decompiler for the control-flow axis | Low | Tooling | Open | Tool list has node/garble/clang/dotnet/r8/nm/objdump but no wasm tool or Ghidra. |
| B-052 | Docker OSS evaluation harness: harden pinned real-world artifacts end to end | Medium | Tooling | Closed | `eval/` fetches pinned artifacts across every ecosystem and runs plan/build/verify with logs + readable output; on-demand, network confined to the harness (ADR-0021, K-046); `asar` surfaces B-043. |
| B-053 | Go and .NET assemblies are misdetected by the 4 KiB sniff window | High | Adapters | Closed | Fixed: `detect_kind` streams the file (bounded) for format markers beyond the sniff prefix; fzf→`go`, Newtonsoft.Json→`dot-net` in the test bowl (K-047). Found by B-052. |
| B-054 | `TargetKind` JSON/manifest name diverges from `as_str()` (`java-script`/`dot-net`) | Low | Tooling | Closed | Fixed: explicit serde renames so JSON and `kind =` use `native-macho`/`dotnet`/`javascript`, matching `as_str()`; test `serialized_kind_matches_as_str` guards it (K-048). Found by B-052. |

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
- **B-013 (Closed, commit 21):** deceptive decoys bounded and sanitized, recorded
  in ADR-0016.
- **B-001 (Closed, commit 22):** generic external-adapter discovery + the LLVM
  reference adapter (K-027).
- **B-005 (Closed, commit 23):** `.NET` reference adapter (`direa-clr`).
- **B-006 (Closed, commit 24):** JVM/Android reference adapter (`direa-jvm`).
- **B-023 (Closed, commit 25):** behavior-safe JS runtime guard embedded by
  `build` (K-028); native embedding opened as B-027.
- **B-024 (Closed, commit 26):** external detector hook + broader corpus
  (K-029).
- **B-025 (Closed, commit 27):** real-toolchain CI lanes + `docs/ADAPTERS.md`.
- **B-026 (Closed, commit 28):** external decompiler-based control-flow axis
  (ADR-0017, K-030).
- **B-027 (Closed, commit 29):** linkable native guard library (ADR-0018,
  K-031).
- **B-028 (Closed, commit 30):** CLI verbosity, timing and transform trace
  (K-032).
- **B-029 (Closed, commit 31):** stable error codes + JSON error envelope
  (K-033).
- **B-030 (Closed, commit 32):** `die-a-rea doctor` (K-034).
- **B-031 (Closed, commit 33):** `die-a-rea explain <id>` (K-035).
- **B-032 (Closed, commit 34):** toolchain-free WASM + Java `.class` post
  (K-036).
- **B-033 (Closed, commit 35):** composite GitHub Action.
- **B-034 (Closed, commit 36):** artifact discovery in `init` (K-037).
- **B-035 (Closed, commit 37):** crates metadata + release workflow + `direa-ffi`
  (K-038).
- **B-036 (Closed, commit 38):** cross-platform guard detection (K-039).
- **B-037 (Closed, commit 39):** SARIF + JUnit report formats (K-040).
- **B-038 (Closed, commit 40):** end-to-end eval harness + bench + RRS regression
  gate (K-041).
- **B-039 (Closed, commit 41):** removed the unmeasured agent-deception axis
  (ADR-0019, K-042).
- **B-040 (Closed, commit 42):** bench corpus across formats (K-043); README
  language-support table.
- **B-041 (Closed, commit 43):** the canary detector normalizes before matching
  and the gauntlet carries a red-team evasion corpus a naive matcher misses
  (ADR-0020, K-044).

# STATE — short-term state

> Keep this file lean. It holds **short-term state only**: the hub, one ordered
> work list, the per-change loop, the skip-list, and the single counters table.
> Running history belongs in the Appendix and `BACKLOG.md` — not in the hub.

## HUB

**Read once, in this order:** `AGENTS.md` → `STATE.md` → `docs/README.md` →
`BACKLOG.md` → `KNOWLEDGE.md`. Then look facts up by id.

**Current state (verified 2026-10-12).** die-a-rea is a Rust Cargo workspace
(`direa-core`, `direa-cli`, `direa-verify`, `direa-ffi`). `cargo build`,
`cargo fmt --check`, `cargo clippy -D warnings` and `cargo test --all` all pass;
`die-a-rea bench --check` and `canary-selftest` are green in CI. The built-in
pipeline hardens JS/tree, ELF/PE/Mach-O (incl. Go), WASM and Java targets and
writes a `die-a-rea.build.json` sidecar; `verify` reports an RRS. External
adapters (LLVM/JavaScript/.NET/JVM) and the runtime guard are implemented. See
"What works end-to-end today" for the verified detail and the Appendix for
history; open work lives in `BACKLOG.md`.

**I want… → Go to**

| I want to… | Go to |
|---|---|
| Know the binding rules | `AGENTS.md` |
| Know what to do next | "Ordered work list" below |
| Why a decision was made | `docs/adr/README.md` |
| Find a known hole | `BACKLOG.md` |
| Avoid a hard-won trap | `KNOWLEDGE.md` |
| Find where a fact lives | `docs/README.md` |
| Hand off to a fresh agent | `docs/CONTINUATION.md` |

**Recently completed — do NOT redo.** No prior version-control history existed;
the installation snapshot (`INSTALL`) is the baseline. Do not redo anything
listed under "What works end-to-end today".

## Ordered work list

One commit-sized unit per entry; each names its acceptance check. Work them top
to bottom unless a dependency says otherwise.

1. **B-001 — LLVM adapter.** Acceptance: `plan` lists `llvm` as available for an
   ELF target and `build` applies a control-flow pass with all tests green.
2. **B-002 — Go adapter (garble + build-info strip).** Acceptance: a Go binary
   built through the adapter contains no `Go build ID` and no source paths.
3. **B-007 — Runtime guard library + `runtime-instrumentation-exposure` axis.**
   Acceptance: a unit test detects a debugger/instrumentation signature and the
   axis appears in the report.
4. **B-005 — .NET adapter (ConfuserEx).** Acceptance: an assembly is protected
   end-to-end and `verify` reports the CLR-safe axes.
5. **B-006 — Android/JVM adapter (R8/ProGuard).** Acceptance: a config is
   generated and applied via subprocess; no copyleft code is linked (ADR-0007).
6. **B-008 — `direa-canary` adapter.** Acceptance: detector-hardened payloads
   are generated and round-trip through `build`.
7. **B-009 — Detector-hardening gauntlet.** Acceptance: the NPS-style detector
   is run against our canaries in CI and reports detection rates.
8. **B-011 — `CONTRIBUTING.md` + `CHANGELOG.md`.** Acceptance: both exist and
   are indexed in `docs/README.md`.
9. **B-012 — Declare MSRV.** Acceptance: `rust-version` is set and the exercised
   toolchain is recorded in `KNOWLEDGE.md`.
10. **B-013 — Bound deceptive `agent_decoys`.** Acceptance: a decision ADR
    records the validated limits (depends on B-009).
11. **B-014 — Strengthen docs-check counter heuristic.** Acceptance: the check
    fails on a planted duplicate counter and passes on the clean tree.
12. **B-015 — Mach-O DWARF/fat `native-post`.** Acceptance: `__DWARF` sections
    and fat containers lose symbol/debug metadata (follow-up to B-004).
13. **B-016 — PE resources/rich-header `native-post`.** Acceptance: PE resource
    and rich-header metadata are stripped (follow-up to B-004).

## Per-change loop

1. **Baseline:** `cargo test --all` green before touching anything.
2. **Change:** code + tests + the durable lesson (`K-NNN`) + docs.
3. **Checks:** `cargo fmt --all -- --check`; `cargo clippy --all-targets -- -D warnings`;
   `cargo test --all`; `bash tools/docs-check.sh`.
4. **Commit:** `<area>: <imperative summary> (<ID>)`, body ending in a
   `Doc impact:` line.
5. **Push:** only when explicitly asked.

## What works end-to-end today (verified)

- `cargo build` — clean.
- `cargo test --all` — all tests pass (see the counters table).
- `die-a-rea init --name X <target>` writes a starter manifest.
- `die-a-rea plan` / `plan --json` lists per-target adapters and missing ones.
- `die-a-rea build --out DIR [--seed N]` hardens JS/tree targets, injects seeded
  decoys + bounded tarpits, and writes `DIR/die-a-rea.build.json`.
- `die-a-rea build` on a native ELF/PE/Mach-O target runs the built-in
  `native-post` stripper (symbols/debug/build metadata); e.g. a `gcc -g` ELF loses
  `.symtab`/`.debug_*` and the debug axis moves 1.00 -> 0.00 (ADR-0008). Go
  binaries also lose their build-info blob and build ID (B-002, K-019).
- `die-a-rea build` on a JS target runs the external `js` adapter when
  `DIREA_JS_ADAPTER` or `direa-js` is available (ADR-0009); e.g. a bundle
  obfuscated by `javascript-obfuscator`. Absence is a note, not an error.
- `die-a-rea build` on a `.jar` unpacks the zip in-process, runs the built-in
  `java-post` on each `.class`, and repacks it; the `debug-info` axis is
  jar-aware, so the win is measured (B-058, K-057). An archive the rewriter
  cannot parse is copied and deferred to the `jvm` adapter.
- `die-a-rea verify --original A --hardened B` prints an RRS report; `--json`
  for CI; `--with-rea` runs the external adversary when available; `--with-runtime`
  adds the runtime-instrumentation axis (debugger/Frida/inspector detection). The
  `strings` axis measures verbatim survival of baseline tokens (`direa_core::scan`
  token sets), so minified bundles, encoding and stripping all score correctly
  (ADR-0011).

## SKIP-LIST — do not start without a new decision

- **S-1 Destructive runtime responses** (wiping memory, corrupting control flow,
  refusing to run). Requires a new ADR; conflicts with `docs/ETHICS.md`.
- **S-2 Deceptive agent payloads by default** (`agent_decoys`). Gated on the
  detector gauntlet (B-009) and a new ADR; see ADR-0006.
- **S-3 Protecting artifacts you are not authorized to protect.** Prohibited;
  `docs/ETHICS.md`.
- **S-4 Any network/upload/telemetry feature.** Contradicts ADR-0001.
- **S-5 Re-weighting RRS axes to inflate scores.** Invalidates the measurement;
  ADR-0004, ADR-0005.

## Counters (canonical — the only place these numbers live)

Derived and verified by `tools/docs-check.sh` (check 6); update a value in the
same change that moves it. Git owns the commit history — it is deliberately not
counted here (one fact, one home).

| Metric | Value |
|---|---|
| Rust crates | 4 |
| Tests (`cargo test --all`) | 124 |
| Backlog items (`B-NNN`) | 59 |
| Open backlog items (High) | 0 |
| Durable lessons (`K-NNN`) | 57 |
| Decision records (`ADR-NNNN`) | 21 |
| Markdown docs (tracked) | 44 |

## Appendix — history and do-not-redo ledger

- **INSTALL (commit 1):** installed the project memory (AGENTS/STATE/BACKLOG/
  KNOWLEDGE/docs-README/ADR scaffold/CONTINUATION/docs-check/CI). Existing docs
  retained ownership; no application behaviour changed.
- **Do not redo (settled areas):** the whole `0.1.0` feature set — manifest &
  policy profiles; target detection; planner/adapter registry; built-in JS
  hardening; seeded randomization; tarpits; build-metadata sidecar; RRS probes
  and report. All are covered by tests and listed under "What works".
- **Backfill (commit 2):** `KNOWLEDGE.md` seeded with K-002–K-010 (toolchain facts
  and failure modes evidenced by tests).
- **B-010 (commit 3):** configured `origin`
  (`git@github-die-a-rea:danihelenterprises/die-a-rea.git`, GitHub over 443);
  `docs-check` check 4 now runs against a real commit range. Closed `B-010`.
- **B-004 (commit 4):** built-in `native-post` strips ELF/PE/Mach-O symbol, debug
  and build metadata in-process (ADR-0008, lessons K-011/K-012). Closed `B-004`;
  follow-ups B-015/B-016 opened.
- **B-003 (commit 5):** JavaScript adapter implemented and wired (ADR-0009,
  K-014); run end-to-end against `javascript-obfuscator`. B-003 stays
  In-progress: the metric clause is blocked by the probe blind spot (B-017,
  K-013).
- **B-017 (commit 6):** `strings` probe now counts identifier/literal tokens
  (`direa_core::scan`), fixing the minified-file collapse (ADR-0010, K-013).
  Closed `B-017`; discovered a deeper limit — byte probes cannot see string
  encoding (K-015) — and opened `B-018`.
- **B-018 (commit 7):** `strings` axis redefined as verbatim survival of baseline
  tokens (ADR-0011, K-016), superseding ADR-0005's count formula. Encoding now
  scores without heuristics; closed `B-018` and `B-003` (JS adapter clause now
  passes: 5/9 tokens survive vs 9/9 for the built-in path).
- **B-019 (commit 8):** `build` now refuses to overwrite the source artifact or a
  foreign (non-die-a-rea) output directory; `--force` overrides (ADR-0012,
  K-017). Closed `B-019`.
- **B-020 (commit 9):** `build` health-checks each hardened artifact (non-empty +
  native magic) and runs the opt-in `[verify]` command / `js_syntax`; failures
  fail the build (ADR-0013, K-018). Closed `B-020`.
- **B-002 (commit 10):** built-in Go build-info/ID scrub in `native_post`
  (K-019); `garble` remains bring-your-own for source obfuscation. Closed
  `B-002`.
- **B-007 (commit 11):** `direa_core::guard` + `verify --with-runtime` measure
  runtime instrumentation from the environment (ADR-0014, K-020). Closed `B-007`;
  opened `B-021` (manifest signatures not yet consumed).
- **B-022 (commit 12):** Docker toolchain (`docker/Dockerfile` runtime/dev
  targets, fail-fast `entrypoint.sh`, `compose.yml`), wired into CI (ADR-0015,
  K-021). Closed `B-022`.
- **B-012 (commit 13):** declared MSRV 1.79 workspace-wide (K-022). Closed
  `B-012`.
- **B-011 (commit 14):** added `CONTRIBUTING.md` and `CHANGELOG.md`. Closed
  `B-011`.
- **B-021 (commit 15):** `verify` honors `[runtime] instrumentation_signatures`
  via `guard::detect_with`. Closed `B-021`.
- **B-014 (commit 16):** docs-check counter heuristic made fatal with a broader
  pattern and a `--self-test` in CI. Closed `B-014`.
- **B-016 (commit 17):** PE Rich header stripped; resources preserved (K-023).
  Closed `B-016`.
- **B-015 (commit 18):** Mach-O fat containers + `__DWARF` stripping (K-024).
  Closed `B-015`.
- **B-008 (commit 19):** `direa_core::canary` module wired into `build`
  (K-025). Closed `B-008`.
- **B-009 (commit 20):** detector gauntlet + `canary-selftest` in CI (K-026).
  Closed `B-009`.
- **B-013 (commit 21):** deceptive decoys bounded/sanitized and recorded in
  ADR-0016. Closed `B-013`.
- **B-001 (commit 22):** generic external-adapter discovery + LLVM reference
  adapter (K-027). Closed `B-001`.
- **B-005 (commit 23):** `.NET` reference adapter `adapters/clr/direa-clr`.
  Closed `B-005`.
- **B-006 (commit 24):** JVM/Android reference adapter `adapters/jvm/direa-jvm`.
  Closed `B-006`.
- **B-023 (commit 25):** embedded the JS runtime guard (K-028); opened `B-027`
  (native embedding). Closed `B-023`.
- **B-024 (commit 26):** pluggable external detector + broadened corpus (K-029).
  Closed `B-024`.
- **B-025 (commit 27):** adapter CI lanes + `docs/ADAPTERS.md`. Closed `B-025`.
- **B-026 (commit 28):** external decompiler control-flow axis; proxy removed
  (ADR-0017, K-030). Closed `B-026`.
- **B-027 (commit 29):** linkable native guard library `runtime/guard/`
  (ADR-0018, K-031). Closed `B-027`.
- **B-028 (commit 30):** CLI verbosity/timing/trace (K-032); opened B-029..B-031.
- **B-029 (commit 31):** stable error codes + JSON error envelope (K-033).
  Closed `B-029`.
- **B-030 (commit 32):** `die-a-rea doctor` readiness command (K-034). Closed
  `B-030`.
- **B-031 (commit 33):** `die-a-rea explain <id>` (K-035). Closed `B-031`.
- **B-032 (commit 34):** WASM + Java `.class` built-in post (K-036); opened
  B-033..B-037. Closed `B-032`.
- **B-033 (commit 35):** composite GitHub Action. Closed `B-033`.
- **B-034 (commit 36):** artifact discovery in `init` (K-037). Closed `B-034`.
- **B-035 (commit 37):** crates metadata, release workflow, `direa-ffi` (K-038).
  Closed `B-035`.
- **B-036 (commit 38):** cross-platform guard detection (K-039). Closed `B-036`.
- **B-037 (commit 39):** SARIF + JUnit report formats (K-040). Closed `B-037`.
- **B-038 (commit 40):** end-to-end eval harness + `bench` + RRS regression gate
  (K-041). Closed `B-038`.
- **B-039 (commit 41):** removed the unmeasured agent-deception axis (ADR-0019,
  K-042). Closed `B-039`.
- **B-040 (commit 42):** bench corpus across JS/WASM/Java/ELF + README language
  table (K-043). Closed `B-040`.
- **B-041 (commit 43):** canary detector normalizes before matching; gauntlet
  carries a red-team evasion corpus a naive matcher misses (ADR-0020, K-044).
  Closed `B-041`.
- **Doc-truth (same change):** refreshed the stale HUB current-state paragraph
  and corrected the canonical counters (crates, tracked docs, commit count) as
  part of the B-041 doc-sync.
- **Gap review:** per-ecosystem capability audit opened `B-042`–`B-051`
  (Android/asar/Go/WASM/JVM/guard-parity probes/REA/init/bench/doctor gaps); no
  behaviour change.
- **Counter hygiene (K-045):** `tools/docs-check.sh` check 6 re-derives every
  canonical STATE counter and fails on drift; the `Commits in history` row was
  removed because git owns it.
- **B-052:** added the on-demand Docker OSS evaluation harness `eval/` (pinned
  real-world artifacts across every ecosystem; logs + readable hardened output),
  with ADR-0021 confining network to the harness and K-046. Closed `B-052`. The
  first run surfaced `B-053` (Go/.NET misdetection past the 4 KiB sniff window;
  High) and `B-054` (`java-script` kind naming).
- **Test bowl naming:** the harness is branded **the test bowl** — image
  `die-a-rea-testbowl`, launcher `./testbowl`, output `testbowl-out/`; the source
  stays in `eval/` (so ADR-0021/K-046 remain accurate). No behaviour change.
- **B-053 (K-047):** `detect_kind` streams the file for Go/.NET markers beyond the
  4 KiB sniff window; the test bowl now classifies fzf as `go` and
  Newtonsoft.Json as `dot-net` (was `native-elf`/`native-pe`). Closed `B-053`.
- **B-054 (K-048):** explicit `TargetKind` serde renames so the JSON/manifest
  name matches `as_str()` (`native-macho`/`dotnet`/`javascript`, was
  `native-mach-o`/`dot-net`/`java-script`). Closed `B-054`.
- **B-043 (K-049):** single-file `.asar` is copied verbatim (opaque archive) and
  deferred to an adapter instead of being decoded as UTF-8; the test bowl `asar`
  case now succeeds. Closed `B-043`; opened `B-055` for real built-in asar
  transforms.
- **B-047 (K-050):** the `debug-info` probe is format-aware — native DWARF/PDB,
  JVM debug attributes and WASM custom sections — so `java-post`/`wasm-post`
  wins show up in RRS. Closed `B-047`; opened `B-056` for DEX debug measurement.
  This lifted the bench `wasm-module`/`java-class` scores 50 → 61.
- **B-050 (K-051, K-052):** the bench corpus now also covers PE/Mach-O
  (synthetic, via `bench/gen_fixtures.py`), Go/.NET (real, toolchain-gated) and
  Electron `.asar`/Android APK. Go and .NET were observed in a throwaway
  Go/.NET Docker container (go 41, dotnet 24) and are now gated in
  `expected.json`; all 11 cases pass. Closed `B-050`.
- **B-044 (K-053, K-054):** bundled the `direa-go` (garble) and `direa-wasm`
  (wasm-opt/wasm-mutate) reference adapters, documented them, added optional CI
  lanes, wired `options.out` so `{out}` resolves, and verified both end-to-end
  in a container. Closed `B-044`; opened `B-057` (older adapters inherit tool
  stdout).
- **B-045 (K-055):** added `TargetKind::JavaJar` (detected by `.jar`) and
  registered the `jvm` adapter for `JavaClass`/`JavaJar`/`AndroidApk`, so Java
  `.class` and `.jar` targets now reach an external lane. Closed `B-045`; opened
  `B-058` (built-in jar repack).
- **B-055 (K-056):** `direa_core::asar` rewrites embedded JS and repacks the
  archive; the `.asar` build arm now hardens instead of copying, with a copy
  fallback for unparseable archives. Bench `electron-asar` rose 0 → 35; verified
  with a real `asar extract`. Closed `B-055`.
- **B-058 (K-057):** `direa_core::jar` unpacks a `.jar` (ZIP), runs `java-post`
  on each `.class`, and repacks with fresh local headers/CRCs/offsets; the build
  arm falls back to a verbatim copy for archives it cannot rewrite (ZIP64,
  encryption, exotic methods). The `debug-info` probe is jar-aware and bench
  `java-jar` was added. Closed `B-058`.
- **B-057 (K-053):** the `clr`/`jvm`/`llvm` adapters now capture the wrapped
  tool's stdout/stderr (like `go`/`wasm`) instead of inheriting them, so tool
  output no longer corrupts the JSON response and failures surface it;
  `crates/direa-cli/tests/adapters.rs` guards the behavior. Closed `B-057`.
- **B-051:** `doctor` now reports the WASM toolchain (`wasm-opt`, `wasm-mutate`,
  `wasm2wat`, `wat2wasm`) and common decompilers (`ghidra`, `analyzeHeadless`,
  `radare2`, `rizin`) alongside the adapter tools, so readiness covers the
  control-flow axis (`verify --decompiler`, ADR-0017). Closed `B-051`.
- **B-049:** `discover` (used by `init`) now also finds Go (`go.mod` module
  binary), .NET (`bin`/`publish` assemblies), Android (`build/outputs/apk/*.apk`)
  and C/C++ (CMake/Make/meson `build/` linked binary, objects skipped), with
  budget-bounded recursive scans and deduped results. Closed `B-049`.

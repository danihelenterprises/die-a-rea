# STATE — short-term state

> Keep this file lean. It holds **short-term state only**: the hub, one ordered
> work list, the per-change loop, the skip-list, and the single counters table.
> Running history belongs in the Appendix and `BACKLOG.md` — not in the hub.

## HUB

**Read once, in this order:** `AGENTS.md` → `STATE.md` → `docs/README.md` →
`BACKLOG.md` → `KNOWLEDGE.md`. Then look facts up by id.

**Current state (verified 2026-10-12).** die-a-rea is an early scaffold: a Rust
Cargo workspace with three crates (`direa-core`, `direa-cli`, `direa-verify`).
`cargo build`, `cargo fmt --check`, `cargo clippy -D warnings` and
`cargo test --all` (37 tests) all pass. The built-in pipeline hardens JS/tree
targets (source-map + comment stripping, notice/canary injection, seeded decoys,
bounded tarpits) and writes a `die-a-rea.build.json` sidecar; `verify` reports an
RRS with signal-to-noise and analysis-noise axes. Deep per-ecosystem adapters
(LLVM/Go/JS/.NET/Android) and the runtime guard are **not implemented** (B-001…
B-007). The repository has no remote yet (B-010).

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
- `cargo test --all` — 37 tests pass.
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

| Metric | Value |
|---|---|
| Rust crates | 3 |
| Tests (`cargo test --all`) | 85 |
| Backlog items (`B-NNN`) | 37 |
| Open backlog items (High) | 0 |
| Durable lessons (`K-NNN`) | 36 |
| Decision records (`ADR-NNNN`) | 18 |
| Markdown docs (tracked) | 38 |
| Commits in history | 34 |

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

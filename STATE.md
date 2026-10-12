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

1. **B-010 — Configure a git remote.** Acceptance: `git remote -v` shows an
   `origin`, and `tools/docs-check.sh` check 4 reports a commit range instead of
   skipping.
2. **B-004 — Built-in `native-post` stripping.** Acceptance: after `build` on an
   ELF/Mach-O/PE target, `.symtab`/`.debug_*`/build-id markers are absent and the
   symbol/debug RRS axes are measured (not skipped).
3. **B-001 — LLVM adapter.** Acceptance: `plan` lists `llvm` as available for an
   ELF target and `build` applies a control-flow pass with all tests green.
4. **B-003 — JavaScript/Electron adapter.** Acceptance: a JS bundle round-trips
   through the adapter and string-exposure improves versus the built-in path.
5. **B-002 — Go adapter (garble + build-info strip).** Acceptance: a Go binary
   built through the adapter contains no `Go build ID` and no source paths.
6. **B-007 — Runtime guard library + `runtime-instrumentation-exposure` axis.**
   Acceptance: a unit test detects a debugger/instrumentation signature and the
   axis appears in the report.
7. **B-005 — .NET adapter (ConfuserEx).** Acceptance: an assembly is protected
   end-to-end and `verify` reports the CLR-safe axes.
8. **B-006 — Android/JVM adapter (R8/ProGuard).** Acceptance: a config is
   generated and applied via subprocess; no copyleft code is linked (ADR-0007).
9. **B-008 — `direa-canary` adapter.** Acceptance: detector-hardened payloads
   are generated and round-trip through `build`.
10. **B-009 — Detector-hardening gauntlet.** Acceptance: the NPS-style detector
    is run against our canaries in CI and reports detection rates.
11. **B-011 — `CONTRIBUTING.md` + `CHANGELOG.md`.** Acceptance: both exist and
    are indexed in `docs/README.md`.
12. **B-012 — Declare MSRV.** Acceptance: `rust-version` is set and the exercised
    toolchain is recorded in `KNOWLEDGE.md`.
13. **B-013 — Bound deceptive `agent_decoys`.** Acceptance: a decision ADR
    records the validated limits (depends on B-009).
14. **B-014 — Strengthen docs-check counter heuristic.** Acceptance: the check
    fails on a planted duplicate counter and passes on the clean tree.

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
- `die-a-rea verify --original A --hardened B` prints an RRS report; `--json`
  for CI; `--with-rea` runs the external adversary when available.

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
| Tests (`cargo test --all`) | 37 |
| Backlog items (`B-NNN`) | 14 |
| Open backlog items (High) | 6 |
| Durable lessons (`K-NNN`) | 10 |
| Decision records (`ADR-NNNN`) | 7 |
| Markdown docs (tracked) | 23 |
| Commits in history | 2 |

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

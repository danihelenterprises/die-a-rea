# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project aims to
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Manifest + policy profiles (`low`/`medium`/`high`/`paranoid`) with per-field
  overrides; target detection (ELF, Mach-O, PE, .NET, Go, JS, Electron/ASAR,
  WASM, APK); adapter registry and planner.
- CLI: `die-a-rea init | profiles | plan | build | verify`.
- Built-in `native-post` stripping: ELF/PE/Mach-O symbol, debug and build
  metadata, plus Go build-info/ID scrubbing (ADR-0008, K-019).
- Seeded randomization (decoy functions/strings/files, depth bombs) and tarpits
  (padding, honeypots, honest prompt gate), reproducible via `--seed`.
- Build-metadata sidecar (`die-a-rea.build.json`) enabling decoy-aware scoring.
- Reverse-engineering Resistance Score (RRS): verbatim-survival string axis and
  analysis-noise axis (ADR-0011).
- External JavaScript/Electron adapter (`adapters/js/direa-js`) and build wiring
  via `DIREA_JS_ADAPTER` (ADR-0009).
- Output-safety guards: `build` never overwrites the source or a foreign output
  directory (`--force` to override) (ADR-0012).
- Post-build health check: structural sanity plus opt-in `[verify] command` and
  `js_syntax` (ADR-0013).
- Runtime guard `direa_core::guard` and `verify --with-runtime` for the
  runtime-instrumentation axis (ADR-0014).
- Docker toolchain (`runtime` and `dev` images) with a fail-fast entrypoint
  (ADR-0015).
- Project memory (`AGENTS.md`, `STATE.md`, `KNOWLEDGE.md`, `BACKLOG.md`,
  `docs/adr/`) and the `tools/docs-check.sh` invariant checker.

[Unreleased]: https://github.com/danihelenterprises/die-a-rea/commits/main

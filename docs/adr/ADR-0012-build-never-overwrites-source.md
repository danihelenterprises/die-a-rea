# ADR-0012 — Build never overwrites the source or unrelated files

- **Status:** Accepted
- **Date:** 2026-10-12
- **Deciders:** die-a-rea maintainers
- **Supersedes:** none

## Context

`build` computes `dest = out.join(relative_dest(src))`. With `--out .` (or any
`--out` equal to or inside the source), that would overwrite the source artifact.
It also wrote into an existing output directory without checking whether
die-a-rea created it, risking unrelated files. Applying a hardening tool must
never damage the user's codebase.

## Decision

`build` refuses, with a non-zero exit (not a warning), to:

1. write an output whose resolved path **equals the source artifact**; or
2. write output **inside a source directory** (when the target is a directory); or
3. write into an existing, non-empty `--out` that **lacks a
   `die-a-rea.build.json` sidecar** (i.e. die-a-rea did not create it).

A single `--force` flag overrides (1)–(3). `plan` and `build --dry-run` write
nothing. `verify` only reads. These guards are implemented in
`guard_source_and_output` and the output-directory check in `build`.

## Consequences

### Positive
- The source tree cannot be clobbered by a mistyped `--out`.
- Re-running into a die-a-rea output directory still works (sidecar present).

### Negative / risks
- Reusing a foreign directory requires an explicit `--force`.
- Path comparison is best-effort (canonicalise when the path exists, otherwise
  absolute); symlink trickery is out of scope.

## Ergonomics

Default flow is safe: use the default `--out dist-hardened` (or any fresh dir).
Errors name the offending path and the override. Evidence:
`crates/direa-cli/src/build.rs` (`guard_source_and_output`, test
`refuses_output_over_source`); lesson K-017.

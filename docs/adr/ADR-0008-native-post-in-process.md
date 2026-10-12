# ADR-0008 — Built-in `native-post` strips metadata in-process, conservatively

- **Status:** Accepted
- **Date:** 2026-10-12
- **Deciders:** die-a-rea maintainers
- **Supersedes:** none

## Context

The planner registers `native-post` as a `builtin` adapter for ELF/Mach-O/PE, but
`build` was copying native artifacts unchanged — so native targets received no
signal reduction at all, and the symbol/debug RRS axes were never exercised.
Native hardening could either (a) shell out to `strip`/`llvm-strip`, adding a
toolchain dependency outside the adapter boundary, or (b) be done in-process.

## Decision

`crates/direa-core/src/native_post.rs` strips metadata in-process and
conservatively:

- **Formats:** ELF (section headers), PE (COFF symbol table + debug directory +
  timestamp), Mach-O (`LC_SYMTAB` symbol and string tables).
- **Only metadata is touched.** Code, relocations, and dynamic symbol tables
  (`.dynsym`) are left intact so behaviour and loading are unchanged.
- **Names are scrubbed, not just data**, because the verify probes match literal
  marker bytes (`.symtab`, `.debug_info`, `.pdb`).
- **Fail-closed and non-destructive:** every access is bounds-checked; on an
  unsupported or truncated layout the buffer is left untouched and the report
  has `changed == false`.
- **Scope limits:** Mach-O DWARF sections/segment names, PE resources, and
  Mach-O fat (universal) containers are out of scope for now.

## Consequences

### Positive
- Native targets are hardened with no external tools; `native-post` stays
  `builtin` and the symbol/debug axes become measurable.
- No new dependency; license-clean (ADR-0007).

### Negative / risks
- Appending a tarpit blob or editing a signed binary invalidates code
  signatures; re-signing remains the operator's responsibility.
- Best-effort coverage: some metadata survives on Mach-O.

## Ergonomics

Adding a format means adding a handler with bounds checks and tests, never
loosening existing ones. Evidence: `crates/direa-core/src/native_post.rs`
(unit tests); wired in `crates/direa-cli/src/build.rs` (`harden_native`); lessons
K-011, K-012.

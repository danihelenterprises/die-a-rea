# ADR-0013 — Build runs a post-build health check

- **Status:** Accepted
- **Date:** 2026-10-12
- **Deciders:** die-a-rea maintainers
- **Supersedes:** none

## Context

Hardening transforms can break an artifact: `native-post` rewrites metadata,
tarpits append bytes (invalidating code signatures), and the JS path edits and
appends. Shipping a broken hardened artifact — instead of the working original —
is worse than shipping nothing. No check existed.

## Decision

After writing each target (and never during `--dry-run`), `build` runs a
post-build health check:

1. **Always-on structural check.** The hardened artifact is non-empty, and native
   formats (ELF/PE/Mach-O/Go) still carry the source's executable magic.
2. **Opt-in `[verify]` section:**
   - `command` — a shell command run once per target; `{artifact}` is replaced
     with the hardened path and `{out}` with the output directory. A non-zero exit
     fails the build.
   - `js_syntax` — run `node --check` over hardened `.js`/`.mjs`/`.cjs` files when
     `node` is on `PATH` (skipped with a note if it is not).

Any failure is a **hard error** (non-zero exit). `build` therefore fails rather
than publishing a broken artifact.

## Consequences

### Positive
- A broken hardened artifact cannot be shipped silently.
- Language-agnostic: `command` lets any project plug in its own smoke test.

### Negative / risks
- `command` executes a shell string from the user's own manifest — the user's
  responsibility; it is their config, not remote input.
- `js_syntax` can false-positive on ESM files whose directory lacks
  `"type": "module"`; it is opt-in for that reason.

## Ergonomics

Add to the manifest:

```toml
[verify]
command = "node --check {artifact}"   # or any smoke test
js_syntax = true
```

Evidence: `crates/direa-cli/src/build.rs` (`post_build_check`, tests
`post_build_check_fails_on_bad_command`); lesson K-018.

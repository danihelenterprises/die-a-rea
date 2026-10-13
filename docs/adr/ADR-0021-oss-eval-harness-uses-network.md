# ADR-0021 — The OSS eval harness may use network; the product stays local-only

- **Status:** Accepted
- **Date:** 2026-10-12
- **Deciders:** die-a-rea maintainers
- **Supersedes:** none

## Context

We want to test how well `die-a-rea` hardens **real** open-source artifacts
across every supported ecosystem. Obtaining them requires downloading pinned
release artifacts (or, later, cloning and building repos), which is network
egress. ADR-0001 and the skip-list (S-4) forbid network egress *in the product*:
`die-a-rea` must read only local artifacts and never upload or phone home.

The two concerns are separable. Build tooling already uses the network (CI pulls
crates, npm packages and Docker base images); a test harness that fetches its
*inputs* is the same class of activity. What must never happen is the product
binary itself making a request about an artifact.

## Decision

- **The product remains local-only (ADR-0001 unchanged).** `die-a-rea` gains no
  network code, flags, or endpoints. The eval harness invokes the same CLI
  locally and only over local files.
- **Network is confined to `eval/`,** an on-demand development harness. It is not
  wired into the default CI pipeline (run explicitly via `eval/run.sh`).
- **The harness never executes the target.** It configures no `[verify]`
  command, so untrusted third-party artifacts are only read byte-for-byte, never
  run — consistent with "`verify` observes; it must never execute the target".
- **Pinned inputs only.** Each case pins an exact version/URL so runs are
  reproducible and auditable; downloaded artifacts are covered by
  `docs/PIGGYBACK.md` and are never redistributed.
- **Copyleft artifacts stay unlinked (ADR-0007).** A GPL artifact (the Termux APK)
  may be downloaded and hardened as a test input; no copyleft code is ever linked
  into die-a-rea.

## Consequences

### Positive
- We can measure real-world hardening quality on recognizable projects without
  weakening the product's local-only guarantee.
- The harness doubles as an integration test surface for the built-in pipeline
  and auto-detection.

### Negative / risks
- The harness is network-dependent and can break when upstream URLs move; pinned
  URLs mitigate but do not eliminate this.
- Third-party artifacts carry their own licenses; they must never be committed or
  redistributed (only downloaded into a gitignored `eval-out/`).

## Ergonomics

Run `eval/run.sh [case…]`; read `eval-out/REPORT.md` and `eval-out/<id>/`. Add a
case by dropping `eval/repos/<id>.sh` (see `eval/README.md`). Do not add network
calls anywhere outside `eval/`, and do not wire the harness into the default CI
workflow. Evidence: `eval/` (harness), `docs/PIGGYBACK.md` (third-party list).

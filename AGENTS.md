# AGENTS.md — binding working rules

This is the constitution for anyone (human or AI agent) working on **die-a-rea**.
It is deliberately short. Status and work order live in `STATE.md`; durable
lessons in `KNOWLEDGE.md`; open problems in `BACKLOG.md`; decisions in
`docs/adr/`. When a fact has a home, use that home.

## Read order for a fresh session

1. `AGENTS.md` (this file) — the rules.
2. `STATE.md` — current state, the ordered work list, the skip-list.
3. `docs/README.md` — the authority map.
4. `BACKLOG.md` — open items (B-NNN).
5. `KNOWLEDGE.md` — durable lessons (K-NNN).

Then **look facts up by id**. Never load tracked documents whole when a section
will do.

## Non-negotiables (traps the layout cannot reveal)

- **This tool protects software the operator owns or is authorized to protect.**
  It must never be turned into a way to hide malicious behaviour. See
  `docs/ETHICS.md`. (Decision: ADR-0006.)
- **Local-only.** `die-a-rea` reads local artifacts. No network egress, no
  upload, no telemetry. (Decision: ADR-0001.)
- **`verify` observes; it must not execute or mutate the target.** Static probes
  read bytes; the only external process is the optional REA adversary, which is
  a separate tool. (Decision: ADR-0001.)
- **No artifact can be made un-reverse-engineerable.** Marketing that claims
  otherwise is a bug. Measurement, not promises. (Decision: ADR-0004.)
- **Randomization must stay deterministic.** All randomness flows through
  `direa_core::Rng`; do not introduce unseeded randomness. Changing the seed
  derivation changes build output. (Decision: ADR-0003; lesson K-001.)
- **Measurement must not be gamed.** Do not change RRS axis weights to inflate a
  score; change the axis only with a new ADR. (Decision: ADR-0005.)
- **Never link copyleft or proprietary code.** Copyleft/proprietary toolchains
  run behind the adapter subprocess boundary. (Decision: ADR-0007.)

## Authority: one fact, one home

Each kind of fact has exactly one owner file. On conflict the owner wins and the
other file is corrected in the same change.

The authoritative **fact → owner map** and the **doc-sync checklist** live in
`docs/README.md`. Do not restate the map elsewhere; point to it.

## Doc-sync (mandatory)

Every change leaves every document true. Every commit body ends with either:

```
Doc impact: none — <concrete reason>
```

or a list of the documents touched and what changed.

If you learn durable knowledge, mint or amend a `K-NNN` entry **in the same
change**. If you make a decision that constrains future work, add an ADR.

**README currency.** When a change alters anything user-facing — installation, the
CLI, manifest options, adapters, or the setup workflow — update `README.md`
(including the **vibe-coder prompt** at its bottom, when the setup steps change)
in the same change. If nothing user-facing changed, no README update is needed;
say so plainly in the `Doc impact:` line.

## Workflow

1. Pick the next item from the ordered work list in `STATE.md`, or open a new
   `B-NNN` in `BACKLOG.md`.
2. Check the SKIP-LIST in `STATE.md` and any frozen areas below. Do not start
   gated work without a superseding decision.
3. Make the change **and its tests**.
4. Record any durable lesson as a `K-NNN` entry.
5. Update affected docs; keep owners authoritative.
6. Declare `Doc impact:`.
7. Run the checks (below) until green.
8. Commit: `<area>: <imperative summary> (<ID>)` — e.g.
   `verify: discount injected runs from string probe (B-004)`.
   Commit one work-list item at a time; the tree is green at every commit.

## Definition of Done

- [ ] Behavior change matches the picked `B-NNN` acceptance check.
- [ ] `cargo build` — clean.
- [ ] `cargo test --all` — all tests pass; new behavior covered by a test.
- [ ] `cargo fmt --all -- --check` — clean.
- [ ] `cargo clippy --all-targets -- -D warnings` — clean.
- [ ] `bash tools/docs-check.sh` — clean.
- [ ] Durable lessons recorded (`K-NNN`); decisions recorded (`ADR-NNNN`).
- [ ] Docs updated; `Doc impact:` line present in the commit body.
- [ ] No skipped work silently dropped; anything undone is a `B-NNN`.

## Agent shell discipline

- Write scripts to a file, then run one short command. No heredocs or long quoted
  blobs on the command line.
- Prefer dedicated read/search/edit tools over `cat`/`grep`/`sed` in the shell.
- Bound every file write; never emit unbounded content you have not inspected.
- Never touch `target/`, lockfiles by hand, or generated outputs.
- Do not amend or rewrite published history. `git push` only when explicitly
  asked (see `STATE.md` per-change loop).

## Commands (verified)

| Purpose | Command |
|---|---|
| Build | `cargo build` |
| Test | `cargo test --all` |
| Format check | `cargo fmt --all -- --check` |
| Lint | `RUSTFLAGS="-D warnings" cargo clippy --all-targets -- -D warnings` |
| Docs invariants | `bash tools/docs-check.sh` |
| Smoke (CI) | see `.github/workflows/ci.yml` (`init`/`build`/`verify` on a temp JS file) |

## Decisions are immutable

Record a decision once as `docs/adr/ADR-NNNN`. To change it, add a **new** ADR
that supersedes the old one and mark the old one `Superseded by ADR-NNNN`. Never
rewrite or delete an ADR or a `K-NNN` entry.

## Frozen areas

- `docs/adr/` — append-only.
- `KNOWLEDGE.md` — append-only (amend spelling only; never repurpose an id).
- `LICENSE` — MIT; changing it requires a superseding ADR.
- `crates/*/src/**` public contracts (adapter protocol, manifest schema, RRS
  axes) — changing these requires an ADR and matching docs.

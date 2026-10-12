# docs — authority map and doc-sync

This directory holds the durable, design-level documentation. It does **not**
hold current status (that is `../STATE.md`) or working rules (`../AGENTS.md`).

**Read order for a fresh session:** `../AGENTS.md` → `../STATE.md` →
`docs/README.md` (this file) → `../BACKLOG.md` → `../KNOWLEDGE.md`. Then look
facts up by id; never load a tracked document whole when a section will do.

## Document index

Every tracked document, what it owns, and its role.

| Path | Owns | Role |
|---|---|---|
| `../AGENTS.md` | Working rules (the constitution) | rules |
| `../STATE.md` | Current state, ordered work list, skip-list, counters | state |
| `../KNOWLEDGE.md` | Durable lessons (`K-NNN`) | knowledge |
| `../BACKLOG.md` | Open-problem registry (`B-NNN`) | backlog |
| `../README.md` | User-facing overview and pitch | overview |
| `docs/README.md` | Authority map and doc-sync checklist (this file) | meta |
| `docs/CONTINUATION.md` | Reusable fresh-agent handoff prompt | meta |
| `docs/THREAT-MODEL.md` | Adversary, assets, trust boundaries, non-goals | design |
| `docs/REA-PIPELINE.md` | How REA works and where we intervene | design |
| `docs/RANDOMIZATION.md` | Randomization behavior and guarantees | design |
| `docs/TARPITS.md` | Tarpit techniques and safety caps | design |
| `docs/RRS.md` | Reverse-engineering Resistance Score semantics | metric |
| `docs/ETHICS.md` | Responsible use and prohibitions | policy |
| `docs/PIGGYBACK.md` | Prior art and third-party license compatibility | policy |
| `docs/adr/README.md` | ADR index and immutability rule | decisions |
| `docs/adr/0000-template.md` | ADR template | decisions |
| `docs/adr/ADR-NNNN.md` | Individual immutable decisions | decisions |

## Authority map (fact → owner)

On conflict the owner wins and the other file is corrected in the same change.

| Fact | Owner |
|---|---|
| Working rules | `AGENTS.md` |
| Current state, ordered work list, skip-list, counters | `STATE.md` |
| Durable lessons | `KNOWLEDGE.md` |
| Open problems | `BACKLOG.md` |
| Design rationale (immutable) | `docs/adr/` |
| Threat model | `docs/THREAT-MODEL.md` |
| REA pipeline description | `docs/REA-PIPELINE.md` |
| Randomization behavior | `docs/RANDOMIZATION.md` |
| Tarpit behavior | `docs/TARPITS.md` |
| RRS metric semantics | `docs/RRS.md` |
| Responsible use | `docs/ETHICS.md` |
| Third-party licenses / prior art | `docs/PIGGYBACK.md` |
| Commands (build/test/lint/docs) | `AGENTS.md` |

## Doc-sync checklist

Every change leaves every document true. Walk this list before committing:

- [ ] Does the change alter a **metric**? → `docs/RRS.md`.
- [ ] Does it alter **randomization** or seeds? → `docs/RANDOMIZATION.md`, plus a
      `K-NNN` if the seed derivation changed.
- [ ] Does it alter **tarpits**/padding/caps? → `docs/TARPITS.md`.
- [ ] Does it add or change an **adapter** or the adapter protocol? → a new or
      superseding `ADR`, plus `docs/PIGGYBACK.md` if a new dependency appears.
- [ ] Does it change **responsible-use** boundaries? → `docs/ETHICS.md` (+ ADR).
- [ ] Did you learn a **durable lesson**? → a `K-NNN` entry (same change).
- [ ] Did you make a **constraining decision**? → a new `ADR-NNNN` (never edit an
      existing one).
- [ ] Did any number change? → update only the canonical table in `STATE.md`.
- [ ] Is there open work left? → a `B-NNN` row in `BACKLOG.md`.
- [ ] Does every commit body end with `Doc impact: …`?

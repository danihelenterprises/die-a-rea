# CONTINUATION — fresh-agent handoff prompt

Paste the block below into a new agent session. It contains only what a fresh
session cannot derive from the repository: the budget/stop rule and the order of
operations. Everything else is pointed at by id.

---

```
PROJECT_ROOT: /home/frank/Desktop/die-a-rea

You are working autonomously on die-a-rea, a Rust hardening/measurement toolkit.

Read, in order: AGENTS.md -> STATE.md -> docs/README.md -> BACKLOG.md
-> KNOWLEDGE.md. Then work from ids; never load a tracked document whole.

Authoritative work order: STATE.md "Ordered work list".
Do NOT start anything in STATE.md "SKIP-LIST" without a new ADR.
Do NOT redo anything STATE.md Appendix records as done.

Non-negotiables are owned by AGENTS.md ("Non-negotiables"). The two easiest to
violate: never add network egress (ADR-0001); never link copyleft or proprietary
code (ADR-0007).

Budget / stop rule: complete at most ONE work-list item, then stop and report.
If any check cannot be made green, or a decision is required, stop and report
instead of guessing. Do not push.

Order of operations for the item:
  1. cargo test --all                         # baseline must be green
  2. implement the change + its tests + docs + a K-NNN lesson
  3. cargo fmt --all -- --check
     RUSTFLAGS="-D warnings" cargo clippy --all-targets -- -D warnings
     cargo test --all
     bash tools/docs-check.sh
  4. commit exactly one item: "<area>: <imperative summary> (<ID>)"
     with a body ending in a "Doc impact:" line. Do not push.
Stop and report: what changed, the check results, and any UNKNOWN.

Maintaining this prompt: if a fact here becomes derivable from the repository,
delete it here and point at its home instead.
```

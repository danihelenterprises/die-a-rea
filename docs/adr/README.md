# Architecture Decision Records (ADRs)

Immutable records of decisions that constrain future work. One decision per
file, id `ADR-NNNN`, never reused.

## Rules

- **Immutable.** Never rewrite or delete an ADR.
- **Supersede, don't edit.** To change a decision, add a new ADR, mark the old one
  `Superseded by ADR-NNNN`, and leave its text intact.
- New ADRs start from `0000-template.md`.
- The decision must be reflected in the owning docs (`docs/README.md` maps facts
  to owners).

## Index

| ID | Title | Status |
|---|---|---|
| [ADR-0001](ADR-0001-local-only-read-only.md) | Local-only, read-only analysis | Accepted |
| [ADR-0002](ADR-0002-adapter-subprocess-protocol.md) | Adapters are subprocesses over a JSON protocol | Accepted |
| [ADR-0003](ADR-0003-deterministic-randomization.md) | Deterministic, seeded randomization | Accepted |
| [ADR-0004](ADR-0004-measure-with-rrs.md) | Measure resistance with RRS; never claim unbreakability | Accepted |
| [ADR-0005](ADR-0005-signal-to-noise-and-skip-axes.md) | Signal-to-noise strings and skip inapplicable axes | Superseded by ADR-0011 |
| [ADR-0006](ADR-0006-honest-agent-defense.md) | Honest-by-default agent defense; deception opt-in | Accepted |
| [ADR-0007](ADR-0007-mit-core-subprocess-boundary.md) | MIT core; copyleft/proprietary tools behind the subprocess boundary | Accepted |
| [ADR-0008](ADR-0008-native-post-in-process.md) | Built-in `native-post` strips metadata in-process, conservatively | Accepted |
| [ADR-0009](ADR-0009-build-executes-external-adapters.md) | Build executes external adapters opportunistically | Accepted |
| [ADR-0010](ADR-0010-strings-axis-counts-tokens.md) | The `strings` axis counts identifier/literal tokens | Accepted |
| [ADR-0011](ADR-0011-string-exposure-measures-verbatim-survival.md) | String exposure measures verbatim survival | Accepted |

Template: [0000-template.md](0000-template.md).

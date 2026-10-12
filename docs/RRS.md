# Reverse-engineering Resistance Score (RRS)

RRS is a 0–100 measure of how hard a hardened artifact is for automated and
LLM-driven tooling to recover useful meaning from. It is implemented in
`direa-core::rrs` and produced by `direa-verify`.

## Model

Each **axis** is measured as an *exposure* in `[0, 1]`:

- `1.0` — fully recovered by the adversary (e.g. every source map is present).
- `0.0` — nothing recoverable (e.g. no symbol table, no debug info).

The score is the weighted mean of per-axis resistance `(1 - hardened_exposure)`,
scaled to 0–100. Baseline exposure is recorded for the delta display but does
not affect the score.

| Axis | Default weight | Probe |
|---|---:|---|
| `string-exposure` | 1.5 | signal-to-noise of printable sequences (`strings` proxy) |
| `symbol-exposure` | 1.0 | symbol/relocation section markers (`nm` proxy) |
| `debug-metadata-exposure` | 0.8 | DWARF/PDB markers (boolean) |
| `source-map-exposure` | 1.2 | `*.map`, `sourceMappingURL`, `sourcesContent` |
| `control-flow-exposure` | 1.5 | x86 function-prologue signatures |
| `analysis-noise` | 1.5 | injected decoy/tarpit fraction (from build metadata) |
| `runtime-instrumentation-exposure` | 1.0 | guard-lib result (planned) |
| `agent-deception` | 2.0 | REA adversary / canary layer (experimental) |

Weights are `Axis::weight` and can be overridden per measurement.

### Signal-to-noise for strings

An analyst can always read strings; decoys do not remove them. So when build
metadata is present, `string-exposure` measures `real / (real + injected)`
instead of raw count ratio. Adding 500 decoy strings to a file with 8 real
strings yields an exposure near `0.02`, i.e. almost all of what an analyst
recovers is noise.

### Inapplicable axes are skipped

A count probe whose baseline is zero (e.g. symbol markers in a JavaScript file,
or x86 prologues on ARM) is **omitted**, with a note, rather than counted as
fully resisted. This prevents an irrelevant axis from inflating the score.

## Normalisation

- **Count probes:** `baseline_exposure = 1.0` when the baseline count is
  non-zero; `hardened_exposure = clamp(hardened / baseline)`. So dropping strings
  from 1000 to 10 yields an exposure of `0.01`.
- **Boolean probes:** the value passes through directly (`0.0` or `1.0`).

Axes are omitted entirely from a report when they cannot be measured (for
example, runtime instrumentation without a live target), rather than being
silently counted as fully resisted.

## Grades

| RRS | Grade |
|---:|---|
| 90–100 | A |
| 75–89 | B |
| 60–74 | C |
| 40–59 | D |
| 0–39 | F |

## Using RRS in CI

```bash
die-a-rea verify --original dist/app.js --json --out direa-report.json
```

Fail the build when the score regresses:

```bash
rrs=$(die-a-rea verify --original dist/app.js --json | jq .rrs)
test "$rrs" -ge 75 || { echo "RRS regression: $rrs"; exit 1; }
```

## Caveats

The built-in probes are transparent **proxies**, not a full decompiler. They are
deliberately dependency-free so results are reproducible. External adapters can
contribute additional, stronger measurements (e.g. actual decompiler output
fidelity) through the `verify` adapter operation.

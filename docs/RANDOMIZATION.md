# Randomization

The goal is the 500-piece jigsaw with 300 extra pieces: an analyst (or an agent)
still gets a picture, but it is buried in decoys and can't be trusted as a whole.

## Deterministic by design

Every randomized transformation is driven by a seeded SplitMix64 generator
(`direa-core::random::Rng`). That means:

- **Reproducible builds.** The same seed produces the exact same layout. Pass
  `--seed <n>`, or set `[randomization] seed = <n>`.
- **Stable defaults.** With `seed = 0`, a seed is derived from project name,
  version and target path, so CI and local builds agree.
- **Different releases, different layouts.** Bump the seed per release (or use
  the derived default with a version bump) so an analyst can't reuse one
  mapping across versions.

## What gets generated

| Generator | Effect |
|---|---|
| Decoy functions | plausible arithmetic/string functions with opaque dead branches |
| Decoy strings | fake API keys, URLs, IPs, tokens — inflate the string pool |
| Honeypots | tantalizing dead code (`validate_license_*`, `decrypt_config_*`) |
| Decoy files (trees) | extra `.js` modules in a random-named directory |
| Depth bombs | deeply nested function chains that raise analysis cost |

All decoys are appended or written separately; **real code is never modified**
by the randomization layer, so runtime behaviour is unchanged.

## Configuration

```toml
[randomization]
enabled = true
seed = 0            # 0 = derived; nonzero = reproducible
junk_density = 0.35 # soft target for decoy share
decoy_functions = 200
decoy_strings = 500
decoy_files = 25
depth_bomb = 3
```

Enable/disable independently of the profile via `[policy.overrides]`:

```toml
[policy.overrides]
randomize = true
decoy_generation = true
```

## Measurement

The build writes a `die-a-rea.build.json` sidecar recording exactly what was
injected. `die-a-rea verify` reads it to:

- discount injected sequences from the string probe, so decoys are not mistaken
  for recovered real content;
- score the **analysis-noise** axis (fraction of the artifact that is injected).

The string axis becomes a **signal-to-noise** measure: `real / (real + decoy)`.
Adding 500 decoy strings to a file with 8 real ones drops string exposure from
`1.00` to roughly `0.02`.

## Limits and caveats

- Decoys increase artifact size and may slow load. See [TARPITS.md](TARPITS.md)
  for the safety caps.
- A resourced analyst can filter decoys (dead-code elimination, string
  heuristics). Randomization raises cost; it does not eliminate the signal.
- Keep `junk_density` and counts sane for mobile/embedded targets.

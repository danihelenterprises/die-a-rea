# KNOWLEDGE — durable lessons

One lesson per entry. Id `K-NNN`, monotonic, never reused. Dense format:

```
**K-NNN — Rule/Invariance.** [context/evidence] -> [implication]. (status)
```

Append-only: amend spelling only; never repurpose an id. Mint a new entry only
for a genuinely novel failure mode, toolchain fact, or invariant — otherwise
extend the nearest entry. No ephemeral hashes/versions (those live in lockfiles);
link an id instead of restating.

---

**K-001 — All randomness flows through `direa_core::Rng`.** A deterministic
SplitMix64 generator; `seed = 0` derives a stable seed from project name +
version + target path, and any nonzero seed is reproducible. [Evidence:
`crates/direa-core/src/random.rs`; tests `deterministic_for_same_seed`,
`derived_seed_is_stable`] -> Never add unseeded randomness or change the seed
derivation: both change build output and break reproducible builds. (ADR-0003)

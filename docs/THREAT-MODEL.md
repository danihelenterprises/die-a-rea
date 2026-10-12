# Threat model

## Adversary

An analyst (or an autonomous agent) using tooling such as
[REA](https://github.com/morluto/rea), GhidraMCP, IDA + MCP bridges, JADX, or
plain `strings`/`nm`/`objdump`, to understand what a distributed artifact does —
**without source access and, in the automated case, without a human in the loop.**

Two adversary tiers, because they have different weaknesses:

| Tier | Capability | Weakness |
|---|---|---|
| **Automated / LLM-assisted** | runs a decompiler, extracts strings/symbols/source maps, summarizes with an LLM | brittle to signal loss, obfuscation, and prompt injection |
| **Determined human** | everything above plus patience, skill and custom tooling | none worth claiming — assume they win eventually |

`die-a-rea` optimizes primarily against **tier 1**, with tier 2 as a cost-raising
side effect.

## Assets

- Proprietary algorithms and business logic.
- Secrets embedded in the binary (API endpoints, keys — though secrets should
  never be embedded; see below).
- Licensing/anti-tamper logic.
- Source maps and debug information that reconstruct source almost exactly.
- The knowledge of *what your product does*, which is often the real asset.

## Trust boundaries

```text
your build machine (trusted)
   │  build + harden
   ▼
distributed artifact (untrusted host)
   │  runs on user's machine (attacker may control it)
   ▼
analyst / agent (adversary)
```

Once the artifact is on the user's machine, the adversary controls the
environment. Every runtime check can be patched out. Every static secret can be
recovered. Design around that: assume compromise, minimize what is recoverable,
and make recovery expensive.

## Attack surfaces and defenses

| Surface | Adversary action | die-a-rea defense | Residual risk |
|---|---|---|---|
| Symbols / exports | `nm`, REA symbol view | strip symbol tables | dynamic symbols needed for linking remain |
| Strings | `strings`, decompiler strings view | encrypt/encode strings, remove comments | decoded at runtime in memory |
| Debug info | DWARF/PDB readers | strip debug sections | none if fully stripped |
| Source maps | `sourcesContent` reconstruction | remove `.map` and inline URLs | already-published maps cannot be recalled |
| Control flow | decompiler pseudocode | CFF, opaque predicates, MBA | runtime behavior recoverable via dynamic analysis |
| Runtime | debugger, Frida, CDP, V8 Inspector | anti-debug / anti-instrumentation | patched out on a controlled host |
| String pool | `strings`, decompiler string view, agent string dump | decoy strings dilute signal-to-noise | resourced analyst filters decoys |
| Analysis cost | agent reads/summarizes the artifact | tarpit padding, depth bombs, honeypots | cost is paid by you too (size, AV) |
| LLM interpretation | agent summarizes artifact text | notice gate, canaries, opt-in decoys | detectable, bypassable (arms race) |
| Build metadata | Go build info, `.comment`, version strings | strip / obfuscate | fingerprints may remain |

## Non-goals

- DRM / license enforcement. `die-a-rea` does not decide who may run your app.
- Protecting secrets that must actually be secret at rest on the user's machine.
  If the app can read it, the user can. Use server-side architecture instead.
- Making malware undetectable. Explicitly prohibited (see [ETHICS.md](ETHICS.md)).
- Defeating a determined expert. Assume they will succeed.

## Assumptions

- The artifact is distributed to hosts you do not control.
- The adversary has the artifact bytes and, for tier 1, an automated pipeline.
- You can rebuild and re-harden your artifact (protection is a build step).
- You own or are authorized to protect the artifact.

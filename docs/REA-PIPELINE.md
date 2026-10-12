# How REA works, and where die-a-rea intervenes

This is a summary of REA's pipeline based on its public documentation
(`docs/architecture.mermaid`, `docs/mcp-contracts.md`, `docs/installation.md`,
its `AGENTS.md`). It is the threat model die-a-rea is designed against.

## REA in one diagram

```text
target artifact
   (ELF/Mach-O/PE, .NET, JS/Electron, APK, WASM, EVM, firmware, ...)
        │
        ▼
provider adapter
   (Hopper | Ghidra | IDA | JADX | WABT | .NET metadata | AST | CDP/inspector)
        │  pseudocode, assembly, symbols, strings, xrefs, CFG, metadata
        ▼
REA Evidence (normalized, local-only)
        │
        ▼
MCP tool result ──► agent LLM context ──► "here is how the app works"
```

REA is **local-only**: it does not upload your app. But the *agent* receives the
tool results, and the model provider has its own data policy. So the sensitive
boundary is: **decompiled text and strings leave your machine inside an LLM
context.**

## Capabilities that matter to a defensive tool

- **Static native analysis** via Hopper/Ghidra/IDA: pseudocode, assembly, typed
  references, call graphs, strings, symbols.
- **Static JavaScript/Electron**: modules, imports, source maps, IPC, ASAR
  unpacking, native add-on relationships. No native engine required.
- **.NET**: PE/CLI metadata, members, IL, native dependencies. Static only.
- **Android**: manifest, classes, decompiled methods (JADX), resources (Apktool).
- **WASM/EVM/Go/Firmware/SQLite/HAR/pcap/Flutter**: format-specific extraction.
- **Runtime**: process capture (PTY), browser via CDP/Playwright, V8 Inspector,
  and Frida support in REA's own verification path.

## Where die-a-rea hits each hop

| Hop | die-a-rea layer | Notes |
|---|---|---|
| Artifact on disk | symbol/debug/metadata stripping, string encryption, packing | reduces what any provider can read |
| Decompilation | control/data-flow obfuscation (LLVM, etc.) | makes pseudocode expensive/inaccurate |
| Runtime capture | anti-debug / anti-instrumentation guard lib | detects debuggers, Frida, CDP, V8 Inspector |
| LLM interpretation | notice gate, canaries, opt-in decoys | the REA-specific layer |
| Whole pipeline | RRS verification harness | runs REA as the adversary and scores recovery |

## Why the agent layer is both the most valuable and the least certain

REA's headline capability is delegating interpretation to an LLM. That makes it
vulnerable to indirect prompt injection in artifact text — the mechanism used by
the NPS papers. But the follow-up paper shows those injections are detectable
(regex + an embedding classifier) and can be obfuscated around again. Treat the
agent layer as an **arms race**, not a guarantee:

- Default: an honest **authorization notice** plus **canaries** (defensive,
  traceable, hard to call malicious).
- Opt-in: **decoy semantics** for authorized red-teaming, with the expectation
  that a determined analyst can unmask them.

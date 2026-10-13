# Prior art and upstream projects

`die-a-rea` deliberately builds on established open-source obfuscation and
anti-analysis work instead of reinventing it. This document tracks what to wrap,
why, and the license constraints.

## License rules

`die-a-rea` is MIT. To stay compatible:

- **Link / vendor freely:** MIT, Apache-2.0, BSD.
- **Shell out only (never link):** GPL/LGPL tools such as ProGuard. The adapter
  runs them as separate processes, which keeps MIT distribution clean. Verify the
  exact terms of each tool before shipping it in a bundle.
- **Never bundle:** commercial tools (VMProtect, Themida, Denuvo, DexGuard).
  Integrate only as user-provided, bring-your-own, like REA does with providers.

## Rust crates linked into the MIT core

Adapter toolchains are the only place copyleft/proprietary code is allowed, and
then only behind a subprocess (ADR-0007). The core links permissively licensed
crates only:

| Crate | What it gives us | License |
|---|---|---|
| [flate2](https://github.com/rust-lang/flate2) (pure-Rust `miniz_oxide` backend) | raw DEFLATE for reading and writing the built-in `.jar` (ZIP) rewriter (K-057) | MIT OR Apache-2.0 |

## Native code (C/C++/Rust/Swift) — LLVM

| Project | What it gives us | License |
|---|---|---|
| [Pluto-Obfuscator](https://github.com/bluesadi/Pluto) | control-flow flattening, bogus CF, MBA, global encryption, `trap-angr` (anti-symbolic-execution) | MIT |
| [O-MVLL](https://github.com/open-obfuscator/o-mvll) | LLVM pass plugin, Python-configurable, mobile (Android/iOS) | Apache-2.0 |
| [obfuscator-llvm](https://github.com/obfuscator-llvm/obfuscator) | the original CFF/BCF/sub passes | mixed/BSD |
| [Hikari](https://github.com/HikariObfuscator/Hikari) | Apple/LLVM add-ons | — (unmaintained) |

Plan: an `llvm` adapter that injects passes at `clang`/`rustc` via
`-fpass-plugin` or `-mllvm`, plus `trap-angr` when the target must resist
symbolic execution.

## Rust

| Project | What it gives us | License |
|---|---|---|
| [obfstr](https://github.com/CasualX/obfstr) | compile-time string/xref obfuscation | MIT |
| [goldberg/rustfuscator](https://github.com/0xor0ne/awesome-list) | LLVM-based Rust obfuscation | MIT |

## Go

| Project | What it gives us | License |
|---|---|---|
| [garble](https://github.com/burrowers/garble) | identifier/package/path obfuscation, literal obfuscation, strips build info and symbols | BSD-3-Clause |

Go matters specifically because REA has a built-in **Go binaries** provider that
reads embedded compiler version, modules and build settings. `garble -literals
-tiny` plus build-info stripping directly defeats that.

## JavaScript / Electron

| Project | What it gives us | License |
|---|---|---|
| [javascript-obfuscator](https://github.com/javascript-obfuscator/javascript-obfuscator) | control-flow flattening, string array encryption, dead code | BSD-2-Clause |
| [terser](https://github.com/terser/terser) | minify/mangle | BSD-2-Clause |
| [bytenode](https://github.com/bytenode/bytenode) | V8 bytecode compilation (hides source) | MIT |
| [asarmor](https://github.com/saenzramiro/asarmor) | corrupt/protect Electron ASAR archives | MIT |

REA's JS/Electron path is static and needs no native engine, so this is the
highest-value, lowest-effort adapter.

## .NET

| Project | What it gives us | License |
|---|---|---|
| [ConfuserEx](https://github.com/mkaring/ConfuserEx) / [Neo-ConfuserEx](https://github.com/puzzithinker/neo-ConfuserEx) | renaming, anti-debug, anti-tamper (method encryption), CFF, constants/resources encryption | MIT |

## Java / Android

| Project | What it gives us | License |
|---|---|---|
| [R8](https://r8.googlesource.com/r8) | shrinking, renaming, optimization | Apache-2.0 |
| [ProGuard](https://github.com/Guardsquare/proguard) | renaming, string encryption via config | GPL-2.0 (subprocess only) |

## WASM

| Project | What it gives us | License |
|---|---|---|
| [Binaryen](https://github.com/WebAssembly/binaryen) (`wasm-opt`, `wasm-mutate`) | optimization and semantics-preserving mutation that confuses analyses | Apache-2.0 |
| [WABT](https://github.com/WebAssembly/wabt) | disassembly/validation | Apache-2.0 |

## Runtime anti-analysis

| Project / technique | What it gives us | License |
|---|---|---|
| [Frida](https://frida.re) detection literature | signatures for `frida-agent`, thread names, ports, `/proc` maps | n/a |
| [phantom-frida](https://github.com/TheQmaks/phantom-frida) | *offensive* counterpart — use to test our detection robustness | MIT |
| Anti-debug via `ptrace`/`PTRACE_TRACEME`, `IsDebuggerPresent`, timing | debugger detection | public domain |
| V8 Inspector / CDP detection | detects REA's runtime capture modes | n/a |

## The adversary (for verification)

| Project | Role |
|---|---|
| [morluto/rea](https://github.com/morluto/rea) | the primary adversary; driven by `direa-verify` |
| [GhidraMCP](https://github.com/LaurieWired/GhidraMCP) | MCP adversary used by the NPS research (issues #140/#141) |
| NPS AutoDAN corpus (arXiv 2605.30667) | prompt-injection payloads to harden against |
| NPS detection model (arXiv 2605.30677) | regex + embedding classifier; the gauntlet our canaries must survive |

## Evaluation test artifacts (inputs only)

The test bowl (the [`eval/`](../eval/README.md) harness) downloads pinned
third-party artifacts to test `die-a-rea` on real code (B-052). They are **inputs
only** — never linked,
bundled or redistributed; they land in the gitignored `eval-out/`. GPL artifacts
(e.g. the Termux APK) are read byte-for-byte and never executed (ADR-0021,
ADR-0007).

| Artifact | Project | License |
|---|---|---|
| jq 1.7.1 binary | [jqlang/jq](https://github.com/jqlang/jq) | MIT |
| fd v10.2.0 binary | [sharkdp/fd](https://github.com/sharkdp/fd) | MIT OR Apache-2.0 |
| fzf v0.55.0 binary | [junegunn/fzf](https://github.com/junegunn/fzf) | MIT |
| axios v1.7.7 bundle | [axios/axios](https://github.com/axios/axios) | MIT |
| Newtonsoft.Json 13.0.3 | [JamesNK/Newtonsoft.Json](https://github.com/JamesNK/Newtonsoft.Json) | MIT |
| Gson 2.11.0 `.class` | [google/gson](https://github.com/google/gson) | Apache-2.0 |
| sql.js 1.10.3 `.wasm` | [sql-js/sql.js](https://github.com/sql-js/sql.js) | MIT |
| Termux v0.118.1 APK | [termux/termux-app](https://github.com/termux/termux-app) | GPL-3.0-only |

## Naming note

`Detect It Easy` (DIE) is a well-known packer/compiler detector. The pun in
`die-a-rea` is intentional: make automated detectors and analysers "die" a
little on your artifact.

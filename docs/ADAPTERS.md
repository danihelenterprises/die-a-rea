# Adapters

Adapters are the language-specific halves of die-a-rea. They are subprocesses
speaking a small JSON protocol over stdio ([ADR-0002](adr/ADR-0002-adapter-subprocess-protocol.md)).
The core discovers **external** adapters for a target kind and runs them after the
built-in transforms; a missing adapter is a note, never an error
([ADR-0009](adr/ADR-0009-build-executes-external-adapters.md), K-027).

## Discovery

For each external adapter `id` (e.g. `js`, `llvm`, `clr`, `jvm`), `build`
resolves the executable from:

1. `DIREA_<ID>_ADAPTER` (path to an executable), then
2. the adapter's command on `PATH` (e.g. `direa-js`).

## Bundled reference adapters

| Adapter | Script | Needs | Env |
|---|---|---|---|
| `js` | `adapters/js/direa-js` | `javascript-obfuscator` | `DIREA_JS_ADAPTER` |
| `llvm` | `adapters/llvm/direa-llvm` | an obfuscating LLVM build (Pluto/O-MVLL) | `DIREA_LLVM_ADAPTER`, `DIREA_LLVM_BUILD` |
| `clr` | `adapters/clr/direa-clr` | a .NET protector (ConfuserEx) | `DIREA_CLR_ADAPTER`, `DIREA_CLR_COMMAND` |
| `jvm` | `adapters/jvm/direa-jvm` | R8/ProGuard | `DIREA_JVM_ADAPTER`, `DIREA_JVM_COMMAND` |
| `go` | `adapters/go/direa-go` | `garble` | `DIREA_GO_ADAPTER`, `DIREA_GO_COMMAND` |
| `wasm` | `adapters/wasm/direa-wasm` | `wasm-opt`/`wasm-mutate` (Binaryen) | `DIREA_WASM_ADAPTER`, `DIREA_WASM_COMMAND` |

The `jvm` adapter is registered for Android APK, Java `.class` and Java `.jar`
targets; the built-in `java-post` runs first on `.class` files, and a `.jar` is
unpacked, hardened per `.class` and repacked before the archive is handed to the
adapter (B-058).

`DIREA_LLVM_BUILD` / `DIREA_CLR_COMMAND` / `DIREA_JVM_COMMAND` /
`DIREA_GO_COMMAND` / `DIREA_WASM_COMMAND` are shell commands with `{artifact}`
(the built artifact) and `{out}` (the output path, same as `{artifact}` for
in-place tools) placeholders. When unset the adapter reports `unavailable` and
the build continues.

```bash
# JavaScript end to end
npm install --global javascript-obfuscator@^5
export DIREA_JS_ADAPTER=$PWD/adapters/js/direa-js
die-a-rea build --out dist-hardened

# .NET (bring your own ConfuserEx project)
export DIREA_CLR_ADAPTER=$PWD/adapters/clr/direa-clr
export DIREA_CLR_COMMAND="Confuser.CLI -n {out}.crproj"

# Go (compile-time garble build; the built-in native-post already scrubs build info)
export DIREA_GO_ADAPTER=$PWD/adapters/go/direa-go
export DIREA_GO_COMMAND="garble -literals -tiny build -o {artifact} ."

# WebAssembly (Binaryen post-processor, runs after the built-in wasm-post)
export DIREA_WASM_ADAPTER=$PWD/adapters/wasm/direa-wasm
export DIREA_WASM_COMMAND="wasm-opt -Oz {artifact} -o {out}"
```

## CI lanes

The `js` lane is real and blocking. `clr`/`jvm`/`llvm`/`go`/`wasm` lanes are
**optional** (`continue-on-error`) because they need heavier toolchains and the
full obfuscators are not trivially installable in CI; they exercise the adapter
with a real toolchain command. See `.github/workflows/ci.yml`.

## Writing an adapter

1. Implement the protocol: read one JSON request from stdin, write one JSON
   response to stdout. Operations: `capabilities`, `plan`, `apply`, `verify`.
2. Report `status: "unavailable"` (not an error) when the toolchain is absent.
3. Register it in `crates/direa-core/src/planner.rs` (`default_registry`) if it
   targets a new kind.
4. Never link copyleft/proprietary code — run it as a subprocess (ADR-0007).

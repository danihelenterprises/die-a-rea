<div align="center">

# die-a-rea

**D.I.E.** — **D**ecompiler **I**nterference **E**ngine
**A.R.E.A** — **A**nti-**R**everse-**E**ngineer **A**nything

*Make the reverse-engineering agent die a little.*

[![Build](https://img.shields.io/badge/build-passing-brightgreen)](#development)
[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange)](#development)
[![RRS](https://img.shields.io/badge/resistance-score-0--100-6f42c1)](docs/RRS.md)
[![PRs](https://img.shields.io/badge/PRs-welcome-brightgreen)](#contributing)
[![Status](https://img.shields.io/badge/status-early%20scaffold-yellow)](#repository-status)

**Harden your own software against automated and LLM-driven reverse engineering — and prove how well it worked.**

</div>

---

## The one-paragraph version

`die-a-rea` builds protection layers into the software you ship so that tools like
[REA ("Reverse Engineer Anything")](https://github.com/morluto/rea), GhidraMCP and
similar agent-assisted pipelines have a much harder time understanding what your code
does. It strips the easy signal, obfuscates what remains, floods the analysis with
seeded decoys, tarpits LLM agents with token-costly noise, and then — unlike every
obfuscator you've used — **measures the result** as a 0–100 Reverse-engineering
Resistance Score.

```text
die-a-rea plan      # show exactly what will change
die-a-rea build     # produce hardened artifacts
die-a-rea verify    # score resistance against the original
```

---

## Why this exists

Software is the work of human hands. It is thousands of small, deliberate decisions
made by people who chose to spend their years building something.

When copying someone else's life's work becomes a single prompt — when a competitor can
reconstruct your product in an afternoon with an agent and a decompiler — the incentive
to build quietly dies. Not with a bang, but with a slow spreadsheet: why invest years
when a faster team can lift the result? The people who lose first are the smallest
teams; the craftspeople; everyone whose livelihood depends on the thing they make being
*yours to make*.

Technology is never neutral. It takes on the character of the people who wield it. This
project is built on a simple conviction:

> **The measure of progress is the dignity of the person, not the speed of the machine.**

So `die-a-rea` is not a weapon and not a wall. It is a way to restore a little friction
to copying, so that building remains worth doing — and to keep the work, and the worth,
in the hands of the human beings who made it. We would rather rebuild the walls together
than raise a tower alone.

*Remaining human is the whole point.*

---

## Highlights

- **Defense in depth across the whole analysis pipeline** — artifact, compiler, runtime, randomization, tarpits, and the agent itself.
- **Measured protection.** A real, reproducible **Reverse-engineering Resistance Score (RRS)** from `die-a-rea verify`, not vibes.
- **Polyglot by architecture.** A tiny adapter protocol covers native, JS/Electron, .NET, Go, WASM, JVM/Android and more, wrapping the best open-source protectors per ecosystem.
- **Reproducible randomization.** Seeded decoy generation means the same build is identical every time — and a per-release seed gives every version a fresh layout.
- **Tarpits for AI.** Token-costly padding, depth bombs and honeypots that make automated, LLM-assisted analysis slower and more expensive.
- **License-clean.** Copyleft and proprietary toolchains run behind the adapter boundary; the core stays MIT.

---

## Quick start

Requires Rust (stable).

```bash
cargo build --release
./target/release/die-a-rea --help
```

In your project directory:

```bash
die-a-rea init --name my-app dist/app.js dist/electron-app
# review and edit die-a-rea.toml, then:
die-a-rea plan                                                   # dry run
die-a-rea build --out dist-hardened --seed 2026                  # harden
die-a-rea verify --original dist/app.js \
                  --hardened dist-hardened/dist/app.js           # score it
```

Optionally point it at the real adversary:

```bash
die-a-rea verify --original dist/app.js --with-rea
```

---

## Docker

No local toolchain needed — build an image and run it against your project:

```bash
docker build -f docker/Dockerfile --target runtime -t die-a-rea:runtime .
docker run --rm -v "$PWD":/work die-a-rea:runtime doctor
docker run --rm -v "$PWD":/work die-a-rea:runtime build --out dist-hardened
```

The `runtime` image ships the CLI, Node and the JS adapter; the `dev` image adds
the Rust toolchain for building/testing. The entrypoint fails fast with clear
errors. See [`docker/README.md`](docker/README.md).

## What measurement looks like

```text
target: dist/app.js
RRS: 96/100 (grade A)
  string-exposure                exposure 1.00 -> 0.00  (delta +1.00)
  source-map-exposure            exposure 1.00 -> 0.00  (delta +1.00)
  analysis-noise                 exposure 1.00 -> 0.00  (delta +1.00)
  - strings: 8 real of 2333 sequences (2325 injected)
  - analysis-noise: 99% injected (72182 of 72626 bytes)
```

`--json` emits a machine-readable report for CI gating — fail the build if your
resistance drops.

---

## How it works

Every stage attacks a different part of the automated pipeline:

| # | Layer | What it does | Examples |
|---|-------|--------------|----------|
| 1 | **Artifact signal** | Strip symbols, debug info, build metadata, source maps | ELF/Mach-O/PE, JS bundles |
| 2 | **Code transformation** | Raise decompiler cost | CFF, opaque predicates, MBA (LLVM/Rust/Go/JS) |
| 3 | **Runtime** | Detect debuggers / instrumentation | `ptrace`, Frida, CDP, V8 Inspector |
| 4 | **Randomization** | Seeded decoys that inflate the puzzle | decoy functions/strings/files, depth bombs |
| 5 | **Tarpits** | Analysis-cost inflation for AI agents | padding, honeypots, authorization gate |
| 6 | **Verification** | Score resistance against the original | RRS report |

### Randomization — the 500-piece puzzle with 300 extra pieces

Seeded decoys ([`docs/RANDOMIZATION.md`](docs/RANDOMIZATION.md)) inject plausible fake
functions, strings, files and honeypots, plus deep nesting that is expensive to trace.
Real code is never modified, so runtime behaviour is unchanged.

```toml
[randomization]
enabled = true
seed = 2026          # 0 = derived; nonzero = reproducible per release
decoy_functions = 200
decoy_strings = 500
decoy_files = 25
depth_bomb = 3
```

### Tarpits — make the agent pay for every token

([`docs/TARPITS.md`](docs/TARPITS.md)) Padding, depth bombs, honey-potted dead code, and
an honest authorization gate surfaced as a real string literal. Bounded by
`max_total_kb` so a bad config can't explode your artifact.

```toml
[tarpit]
enabled = true
padding_kb = 128
max_total_kb = 4096
prompt_gate = true
honeypots = 25
```

The build writes a `die-a-rea.build.json` sidecar, so verification can tell decoys from
real content. That's what turns "strings recovered" into a **verbatim-survival** measure:
how much of the original, readable signal is even still there (ADR-0011).

### Don't ship a broken build

After writing each artifact, `build` health-checks it: non-empty and (for native
formats) still carrying the source's magic. Add your own smoke test and it runs too —
a failure **fails the build**, so a broken hardened artifact never ships.

```toml
[verify]
command = "node --check {artifact}"   # or your app's own smoke test
js_syntax = true
```

`{artifact}` is the hardened path and `{out}` the output directory. See
[ADR-0013](docs/adr/ADR-0013-post-build-health-check.md).

---

## Architecture

```text
die-a-rea/
├─ crates/
│  ├─ direa-core/        # manifest, policy engine, adapter SDK, planner, RRS model
│  ├─ direa-cli/         # the `die-a-rea` binary
│  └─ direa-verify/      # probes, REA adversary, RRS report
├─ adapters/js/          # external JavaScript/Electron adapter (`direa-js`)
├─ policies/             # low | medium | high | paranoid presets
├─ examples/die-a-rea.toml
└─ docs/                 # threat model, RRS, randomization, tarpits, ethics, ...
```

Adapters are language-specific subprocesses speaking a small JSON protocol over stdio.
This keeps the core language-agnostic and license-clean: an adapter may drive a copyleft
or proprietary toolchain without linking it.

```text
die-a-rea (host)  --JSON request-->  adapter (stdin)
die-a-rea (host)  <--JSON response-- adapter (stdout)
```

Operations: `capabilities`, `plan`, `apply`, `verify`. See
[`docs/PIGGYBACK.md`](docs/PIGGYBACK.md) for the upstream projects we build on.

Point the build at the bundled JS adapter with
`export DIREA_JS_ADAPTER=$PWD/adapters/js/direa-js` (or put `direa-js` on
`PATH`); it uses `javascript-obfuscator` when resolvable and is otherwise
skipped with a note (ADR-0009).

---

## Scope and honesty

We would rather be trusted than impressive.

- **No artifact is un-reverse-engineerable.** A determined expert with enough time will
  succeed. `die-a-rea` raises the cost of *automated and LLM-assisted* analysis, and
  measures exactly how much.
- **It is an arms race, not a wall.** Published research already shows agent-level
  defenses can be detected and evaded. Padding and honeypots are more durable than
  deceptive instructions, which are opt-in and experimental.
- **You pay too.** Protection costs size, load time, and sometimes AV reputation. The
  RRS report shows exactly what you bought.
- **Legitimate use only.** Protect software you own or are authorized to protect. This
  is not a tool for hiding malicious behaviour from defenders.
- **It won't touch your codebase.** `build` never edits sources in place; it writes to a
  separate `--out` and refuses to overwrite a source artifact or a directory it did not
  create (pass `--force` to override). `plan` and `build --dry-run` write nothing, and
  `verify` only reads. See [ADR-0012](docs/adr/ADR-0012-build-never-overwrites-source.md).

See [`docs/THREAT-MODEL.md`](docs/THREAT-MODEL.md) and [`docs/ETHICS.md`](docs/ETHICS.md).

---

## Prior art and projects we build on

`die-a-rea` does not reinvent obfuscation. It orchestrates the best open-source work per
ecosystem and adds the parts nobody has: the agent layer and the measured score.

| Ecosystem | Upstream | License |
|---|---|---|
| C/C++/Rust (LLVM IR) | Pluto-Obfuscator, O-MVLL | MIT / Apache-2.0 |
| Rust strings/xrefs | obfstr | MIT |
| Go | garble | BSD-3-Clause |
| JavaScript / Electron | javascript-obfuscator, asarmor, bytenode | BSD-2 / MIT |
| .NET | ConfuserEx / Neo-ConfuserEx | MIT |
| Java / Android | R8, ProGuard | Apache-2.0 / GPL (subprocess only) |
| WASM | wasm-opt, wasm-mutate, WABT | Apache-2.0 |
| Adversary (test) | REA, GhidraMCP, NPS AutoDAN corpus | MIT / research |

The agent-layer threat and its defenses are grounded in peer-reviewed work:

- **arXiv 2605.30667** — *Automatically Attacking Software Reverse Engineering AI Agents.*
- **arXiv 2605.30677** — *Investigating Detection and Obfuscation of Prompt Injection Attacks Against Software RE AI Agents.*

---

## Repository status

An early scaffold focused on architecture and the measurement loop. Implemented today:

- Manifest + policy profiles (`low` / `medium` / `high` / `paranoid`) with per-field overrides.
- Target detection (ELF, Mach-O, PE, .NET, Go, JS, Electron/ASAR, WASM, APK).
- Planning with an adapter registry and missing-adapter reporting.
- Safe hardening: source-map removal, comment stripping, notice/canary injection.
- Built-in `native-post`: in-process symbol/debug/build-metadata stripping for ELF/PE/Mach-O, plus Go build-info/ID scrubbing (ADR-0008, K-019).
- External adapters discovered via `DIREA_<ID>_ADAPTER` or `PATH`: `direa-js` (javascript-obfuscator), `direa-llvm` (Pluto/O-MVLL compile-time hook), plus `.NET`/JVM references; toolchains are bring-your-own (ADR-0009, K-027).
- Runtime guard: `direa_core::guard` detects debuggers/Frida/inspectors; `verify --with-runtime` scores it, `build` embeds a behavior-safe JS guard when `anti_instrumentation` is set, and `runtime/guard/` is a linkable native C guard (ADR-0014, ADR-0018).
- Agent-defense gauntlet: `die-a-rea canary-selftest [--detector CMD]` runs the prompt-injection detector (built-in and optional external) over an adversarial corpus and our own payloads (ADR-0006).
- Seeded randomization: decoy functions/strings/files, depth bombs, reproducible via `--seed`.
- Tarpits: bounded padding, honeypots, honest prompt gate, optional custom payloads.
- Build-metadata sidecar enabling decoy-aware verification.
- Resistance probes + RRS (signal-to-noise and analysis-noise) + optional REA adversary.

**Roadmap**

- External adapters: LLVM/Pluto, garble, javascript-obfuscator, ConfuserEx, R8.
- A runtime guard library with anti-debug / anti-instrumentation.
- A detector-hardening gauntlet that runs the NPS-style prompt-injection detector against our own canaries.

---

## Documentation

- [Threat model](docs/THREAT-MODEL.md)
- [How REA works, and where we intervene](docs/REA-PIPELINE.md)
- [Randomization](docs/RANDOMIZATION.md) · [Tarpits](docs/TARPITS.md)
- [Reverse-engineering Resistance Score](docs/RRS.md)
- [Prior art and upstream projects](docs/PIGGYBACK.md)
- [Responsible use](docs/ETHICS.md)
- [Docker toolchain](docker/README.md) · [Adapters](docs/ADAPTERS.md) · [Contributing](CONTRIBUTING.md) · [Changelog](CHANGELOG.md)

`verify` accepts `--with-runtime` (instrumentation axis), `--with-rea` (external adversary) and `--decompiler "<cmd>"` (real control-flow axis); see [docs/RRS.md](docs/RRS.md). Run `die-a-rea doctor` to see which adapters and tools are available, and `die-a-rea explain K-027` (or an ADR/B id) to read a project-memory entry. Every command takes `-v` (phase timing), `-vv` (transform trace) and `-q` (quiet), and `--error-format json` emits a coded error envelope (`error[code]` in human form).
- [Project memory: authority map and doc-sync](docs/README.md) · [working rules](AGENTS.md) · [state & work order](STATE.md)

---

## Development

```bash
cargo build
cargo test
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

## Contributing

Issues and pull requests are welcome. The most valuable contributions right now are
**new adapters** (one per ecosystem) and **new probes** for the RRS. If you add a
capability, add the measurement that proves it — numbers over marketing.

## License

MIT — see [LICENSE](LICENSE). Bundled or derived third-party tools retain their own
licenses; see [docs/PIGGYBACK.md](docs/PIGGYBACK.md).

## Responsible use

`die-a-rea` is for protecting software you own or are authorized to protect. It must not
be used to hide malware or other malicious behaviour from defenders. See
[docs/ETHICS.md](docs/ETHICS.md).

## For vibe coders — paste this into your AI

Not a security engineer? You don't need to be. Copy the block below into your coding
agent (Claude Code, Cursor, Codex, Gemini CLI, …) from the root of your own project and
let it wire `die-a-rea` in: it installs the tool, finds your shipped artifact, hardens
it, scores the result, and adds a CI gate — then reports back before finishing.

```text
You are integrating die-a-rea (https://github.com/danihelenterprises/die-a-rea) into
THIS project as a build-time hardening + measurement step. Use it only on software the
project owner is authorized to protect.

Do the following, and show me the plan before you finish:

1. Get the CLI:
     git clone https://github.com/danihelenterprises/die-a-rea /tmp/die-a-rea
     cargo build --release --manifest-path /tmp/die-a-rea/Cargo.toml
     export PATH="/tmp/die-a-rea/target/release:$PATH"     # provides `die-a-rea`
2. Find this project's SHIPPED artifacts (the binary, JS/Electron bundle or *.asar,
   .NET assembly, WASM, or APK). Inspect the build/package scripts; ask me if unsure.
3. Create and edit the manifest:
     die-a-rea init --name <project> <artifact-1> [artifact-2 ...]
   In die-a-rea.toml set:
     [policy] profile = "high"
     [agent_defense] canaries = ["<project>-<YYYYMMDD>"]
   Keep [tarpit] defaults unless the artifact is size-sensitive.
4. Preview and harden (this never edits my sources; it writes to a separate directory):
     die-a-rea plan
     die-a-rea build --dry-run          # show what would change, write nothing
     die-a-rea build --out dist-hardened
   Missing adapters are reported, not fatal. Native ELF/PE/Mach-O stripping (incl.
   Go build info) and the JavaScript adapter are built in; the LLVM/.NET/Android
   adapters and `garble` for deep Go obfuscation are optional. If the
   output directory already exists and was not made by die-a-rea, the build refuses —
   use --force only if you are sure.
5. Score it and gate CI:
     die-a-rea verify --original <artifact-1> --hardened dist-hardened/<artifact-1>
   Add a CI step that fails if the RRS drops below the threshold I choose. The metric
   is documented in docs/RRS.md.
   Also add a [verify] block to die-a-rea.toml with a smoke test so a broken hardened
   artifact fails the build, e.g.:
     [verify]
     command = "<how this project checks its own build still works>"
     js_syntax = true   # JS/Electron only
6. JavaScript/Electron only: install `javascript-obfuscator` in this project and point
   the build at the bundled adapter:
     export DIREA_JS_ADAPTER=/tmp/die-a-rea/adapters/js/direa-js
7. Read AGENTS.md, docs/THREAT-MODEL.md and docs/ETHICS.md in the die-a-rea checkout
   before making further changes. Do NOT commit or push until I say so. Report what you
   hardened, the before/after RRS, and anything left undone.
```

> `die-a-rea` raises the cost of *automated* reverse engineering and measures the
> improvement; it does not make software unbreakable. See
> [Scope and honesty](#scope-and-honesty).

<div align="center">
<sub>Built so the people who make things can keep making them.</sub>
</div>

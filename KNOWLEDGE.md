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

**K-002 — Clamp prefix reads to the file length.** `std::fs::read(...).get(..4096)`
returns `None` for files smaller than 4096 bytes, so small artifacts silently
degrade to `TargetKind::Unknown`. Use `b.get(..b.len().min(4096))`. [Evidence:
`crates/direa-core/src/planner.rs`; regression test `plans_short_elf_file`] ->
Any fixed-window prefix read (planner and `build.rs`) must clamp to `len`.
(Open)

**K-003 — `verify` discounts injected string runs, not decoy counts.** The string
axis is signal-to-noise, computed from `InjectedStats::injected_string_runs`
written by `build`. Renaming the probe `"strings"` or dropping that metadata field
silently reverts to raw ratios and mis-scores decoys. [Evidence:
`crates/direa-verify/src/harness.rs`; `crates/direa-core/src/buildmeta.rs`;
`crates/direa-cli/src/build.rs` (`printable_runs`)] -> Keep the probe name and the
metadata field in sync. (ADR-0005)

**K-004 — Inapplicable RRS axes are skipped, not scored.** A probe whose baseline
reading is zero (e.g. x86 prologues on a JS target) or that errors is omitted from
the report, never counted as fully resisted. [Evidence:
`crates/direa-verify/src/harness.rs` (`measure`); test `end_to_end_static`] ->
When adding a probe, consciously decide its baseline-zero semantics. (ADR-0005)

**K-005 — Manifest sections need `deny_unknown_fields` plus struct-level
`#[serde(default)]`.** Without the struct-level default, a partial section such as
`[tarpit]` that omits `custom_payloads` fails to parse (`missing field`). [Evidence:
`crates/direa-core/src/manifest.rs`; test `partial_randomization_section_uses_defaults`]
-> New `Manifest` sections must carry both attributes. (Open)

**K-006 — Adapter protocol is JSON over stdio, versioned per request.** Every
`AdapterRequest` carries `protocol = direa_core::adapter::proto_version()` (currently
`1`); adapters reply with an `AdapterResponse`. [Evidence:
`crates/direa-core/src/adapter.rs`; test `request_round_trips`] -> Incompatible
protocol changes require an ADR and a `proto_version` bump. (ADR-0002)

**K-007 — Injected JS is appended, never spliced.** Decoys, padding and notices are
appended after the real file body; inserting content mid-file risks breaking
template literals and ESM structure. [Evidence: `crates/direa-cli/src/build.rs`
(`harden_text_file`, `generate_js_bundle`)] -> Keep JS transforms additive. (Open)

**K-008 — Injected comment/payload text must be sanitized.** A `*/` inside an
injected block comment closes it early and corrupts the generated JS. [Evidence:
`crates/direa-cli/src/build.rs` (`sanitize_comment`); test `comment_sanitized`] ->
Route all operator-supplied text (`notice_text`, `custom_payloads`) through the
sanitizer. (Open)

**K-009 — Tarpit padding is hard-bounded.** `generate_padding` clamps to
`max_total_kb`, and decoy counts are clamped (`MAX_DECOY_*`, `MAX_DEPTH`), so a
misconfigured manifest cannot produce an unbounded artifact. [Evidence:
`crates/direa-cli/src/build.rs`; test `padding_is_bounded`] -> Keep every new
injection behind a cap. (Open)

**K-010 — The build-metadata sidecar path is conventional.** `build` writes
`<out>/die-a-rea.build.json`; `verify` resolves it via
`BuildMetadata::sidecar_for(hardened)` and matches records by exact path then by
filename. [Evidence: `crates/direa-core/src/buildmeta.rs`; tests `find_by_name`,
`noise_fraction_is_clamped`] -> Changing the output layout or sidecar location
breaks decoy discounting and the analysis-noise axis. (Open)

**K-011 — `native-post` is conservative and never panics.** `strip_symbols`
returns a `StripReport` with `changed == false` on unsupported or truncated
layouts, and never rewrites code or the dynamic symbol table (`.dynsym`).
[Evidence: `crates/direa-core/src/native_post.rs`; tests `unknown_is_untouched`,
`tiny_truncated_input_does_not_panic`] -> Extend coverage by adding formats and
paths with bounds checks, never by loosening existing ones. (ADR-0008)

**K-012 — Native de-stripping must scrub name literals, not just data.** The
verify probes match literal marker bytes (`.symtab`, `.debug_info`, `.pdb`), so
zeroing section data alone leaves the markers visible. ELF names live in
`.shstrtab` and PE PDB paths in the debug data; both must be overwritten.
[Evidence: `native_post.rs` (`scrub_all`); tests `elf_strips_symtab_and_debug`,
`pe_clears_symbol_table_and_debug`] -> Any new format handler must scrub name
strings as well as payload. (ADR-0005, ADR-0008)

**K-013 — The `strings` probe collapses on single-line files.** It counts
contiguous printable runs, so a compact/minified JS file is a *single* run:
string-exposure can read ~1.0 no matter how well the bundle is obfuscated.
[Evidence: local demo — compact `javascript-obfuscator` output produced
"1 real of 1 sequences"] -> The probe needs a token- or literal-aware measure
before it can score JS obfuscation; tracked as B-017. (Open)

**K-014 — External adapters are optional at build time.** `build` resolves
`DIREA_JS_ADAPTER` then `direa-js` on `PATH`; absence or failure is a note, not an
error, and the built-in transforms still apply. [Evidence:
`crates/direa-cli/src/build.rs` (`resolve_js_adapter`, `apply_js_adapter`); test
`external_adapter_protocol_round_trips`] -> Never make an adapter a hard build
dependency. (ADR-0009)

**K-015 — A static token probe cannot measure string *encoding*.**
`javascript-obfuscator`'s base64 string array replaces plaintext with encoded
blobs; token counting still sees them as recoverable tokens, and their volume can
exceed the baseline, so `string-exposure` stays at or above 1.0. [Evidence:
`die-a-rea` JS demo — 196 tokens after obfuscation, exposure 1.00] ->
Encoding-aware discounting (detect and down-weight base64/hex blobs) is required
to score it; tracked as B-018. (Open)

**K-016 — String exposure is verbatim survival, not a count.** Any count ratio
(raw or token) reads ~1.0 for `javascript-obfuscator` output because obfuscation
*adds* identifiers that outweigh the encoded literals. Measuring how many of the
baseline's tokens still appear verbatim drops correctly (demo: 5 of 9 survive).
[Evidence: `crates/direa-verify/src/harness.rs` (`token_set`, strings branch);
ADR-0011] -> Never score the strings axis with a count; extract token sets from
both artifacts and intersect. (ADR-0011)

**K-017 — `build` refuses to clobber source or a foreign output directory.**
Because `dest = out.join(relative_dest(src))`, `--out .` would overwrite the
source. The guard rejects output equal to / inside the source, and a non-empty
`--out` without a `die-a-rea.build.json` sidecar, unless `--force`. [Evidence:
`crates/direa-cli/src/build.rs` (`guard_source_and_output`, output-dir check);
test `refuses_output_over_source`] -> Keep these refusals as errors; do not
downgrade to warnings. (ADR-0012)

**K-043 — The bench corpus covers every built-in format.** Cases: `js-secrets`
(JS file), `js-tree` (JS directory), `wasm-module`, `java-class`, `native-elf`.
Text fixtures are committed; binary ones are produced by `bench/generate.sh` and
gitignored, and `bench` skips a case whose target is absent. [Evidence:
`bench/`; `crates/direa-cli/src/main.rs` (`cmd_bench`); CI runs
`bench/generate.sh && die-a-rea bench --check`] -> Add a fixture when a new
built-in post ships; keep expected.json minimums just below observed RRS.
(ADR-0004)

**K-042 — Do not declare an RRS axis without a probe.** `AgentDeception` shipped
as a weight-2.0 axis that nothing measured, implying a capability that did not
exist; it was removed (ADR-0019). [Evidence: `crates/direa-core/src/rrs.rs`;
`docs/RRS.md`] -> Every `Axis` must be produced by a probe, or be skipped by
design; add the probe first. (ADR-0004, ADR-0019)

**K-041 — `bench` is the end-to-end regression gate.** `die-a-rea bench [--check]`
builds and scores every `bench/*/die-a-rea.toml` fixture (targets made absolute so
`build`'s `relative_dest` resolves), and `--check` fails below `bench/expected.json`.
CI runs it, so RRS cannot silently regress. A `crates/direa-cli/tests/e2e.rs`
integration test drives the real binary (build → verify) via
`CARGO_BIN_EXE_die-a-rea`. [Evidence: `crates/direa-cli/src/main.rs` (`cmd_bench`);
`bench/`; `tests/e2e.rs`] -> Add a fixture when a new protection ships; keep the
gate green. (ADR-0004)

**K-040 — `verify --format` supports SARIF and JUnit.** `direa_verify::report`
renders the ResistanceReport as SARIF 2.1.0 (one result per axis, level from
exposure) or JUnit XML (one test case per axis, failing when exposure > 0.5); the
CLI writes the chosen format to stdout and `--out`. [Evidence:
`crates/direa-verify/src/report.rs` (tests); `crates/direa-cli/src/main.rs`
(`ReportFormat`)] -> Keep the human form as default; machine formats are additive.
(Open)

**K-039 — Guard detection is platform-specific.** Linux reads `TracerPid`/maps;
Windows uses `IsDebuggerPresent`/`CheckRemoteDebuggerPresent`; macOS uses
`sysctl(KERN_PROC_PID)` and `P_TRACED`. The C guard mirrors this with
`#if defined(_WIN32)`/`__APPLE__`. [Evidence: `crates/direa-core/src/guard.rs`
(`platform_findings`); `runtime/guard/direa_guard.c`] -> Keep `classify` as the
portable, testable core; add platform probes behind cfg. (ADR-0014, ADR-0018)

**K-038 — `direa-ffi` is an optional C ABI.** A `cdylib` (`libdirea_ffi`) plus
`direa_ffi.h` expose `direa_version`, `direa_guard_detect` and
`direa_strip_native`; internal deps carry `path` + `version` so crates publish in
order (core → verify → cli → ffi). [Evidence: `crates/direa-ffi/` (tests);
`docs/RELEASING.md`] -> Keep the ABI small and stable; the CLI is the full
interface. (Open)

**K-037 — `init` discovers shipped artifacts.** `direa_core::discover` reads
`Cargo.toml` (target/release|debug/<name>), `package.json` (main/bin/dist/asar),
and `pom.xml`/Gradle (`target/*.jar`, `build/libs/*.jar`), falling back to
`dist/`; `die-a-rea init` with no targets uses it. [Evidence:
`crates/direa-core/src/discover.rs` (tests); `crates/direa-cli/src/main.rs`
(`cmd_init`)] -> Keep discovery heuristic and non-failing; it never replaces an
explicit target. (Open)

**K-036 — WASM and Java `.class` have toolchain-free built-in post.** `wasm-post`
empties custom-section names and zeroes their payloads (framing stays valid);
`java-post` zeroes constant-pool `Utf8` names of debug-only attributes
(SourceFile/LineNumberTable/LocalVariableTable/LocalVariableTypeTable/
SourceDebugExtension) so the JVM skips them — reflection-sensitive attributes
(`Signature`, `MethodParameters`) are left intact. [Evidence:
`crates/direa-core/src/{wasm,java}.rs`; tests; `build` arms] -> Never strip
semantic WASM sections or reflection attributes. (ADR-0008)

**K-035 — `die-a-rea explain <id>` surfaces the project memory.** It reads
`KNOWLEDGE.md` (`K-NNN` blocks), `BACKLOG.md` (`B-NNN` rows) and
`docs/adr/ADR-NNNN-*` from `--root` (default cwd), printing the entry or a coded
`id_not_found`/`memory_not_found` error. [Evidence: `crates/direa-cli/src/main.rs`
(`cmd_explain`, `knowledge_block`, `backlog_row`)] -> Keep ids resolvable from the
memory files; never duplicate entries elsewhere. (Open)

**K-034 — `die-a-rea doctor` reports readiness.** It checks the manifest, each
registry adapter (resolved via `adapter::env_key`/PATH) and optional tools
(node, javascript-obfuscator, go/garble, clang, dotnet, r8/proguard, nm/objdump),
human or `--json`. Adapter ids must match their script and env key (the Android
adapter id is `jvm` → `DIREA_JVM_ADAPTER`/`direa-jvm`). [Evidence:
`crates/direa-cli/src/main.rs` (`cmd_doctor`); `crates/direa-core/src/planner.rs`]
-> Keep adapter id, command and env key consistent. (Open)

**K-033 — Errors carry stable codes; the CLI prints them.** `DireaError::code()`
returns a stable string (e.g. `manifest_parse`, `output_over_source`); build
safety refusals use `DireaError::Refused { code, message }`. The CLI reports
`error[code]: …` or a `{"error":{...}}` envelope via `--error-format json`.
[Evidence: `crates/direa-core/src/error.rs` (test `codes_are_stable`);
`crates/direa-cli/src/main.rs` (`report_error`)] -> Give every new refusal a code;
never a bare string with no code. (Open)

**K-032 — CLI verbosity is `-v` (timing) / `-vv` (trace) / `-q` (quiet).**
Implemented with a tiny `direa-cli/src/ui.rs` `Ui` (no logging dependency):
`info` writes diagnostics to stderr gated by level, `out` writes results to
stdout unless quiet, `timed` reports phase durations. [Evidence:
`crates/direa-cli/src/ui.rs`; `main.rs` command handlers] -> Add diagnostics via
`ui.info`, never bare `eprintln!`/`println!`. (Open)

**K-031 — The native guard is a linkable C library, not injected.** `build` never
patches native binaries; `runtime/guard/direa_guard.c` is compiled and linked by
the program author, mirroring `direa_core::guard`. [Evidence:
`runtime/guard/direa_guard.{h,c}`; `crates/direa-core/src/guard.rs` test
`native_guard_compiles_and_runs`] -> Never attempt post-build injection into
native binaries. (ADR-0018)

**K-030 — The control-flow axis is external and opt-in.** It is no longer a
default probe (the x86-prologue proxy is gone); `verify --decompiler "<cmd>"`
adds `ExternalControlFlowProbe`, which reads an integer function count from the
command's stdout, and the axis is skipped when unconfigured. [Evidence:
`crates/direa-verify/src/probes.rs` (`ExternalControlFlowProbe`); `harness.rs`;
`tools/controlflow-count`] -> Never reintroduce a byte-signature proxy for this
axis. (ADR-0017)

**K-029 — The gauntlet accepts an external detector.** `canary::gauntlet_with`
takes an optional `Fn(&str) -> bool`; a sample counts as detected if the built-in
regex detector **or** the external hook flags it. `die-a-rea canary-selftest
--detector <cmd>` wires a command (stdin = text, exit 0 = injection), letting an
ML/hosted classifier extend coverage without bundling a model. [Evidence:
`crates/direa-core/src/canary.rs` (`gauntlet_with`, test
`external_detector_is_consulted`); `crates/direa-cli/src/main.rs`
(`run_detector`)] -> Keep the built-in detector dependency-free; put ML behind the
hook. (ADR-0006)

**K-028 — The runtime guard is embedded into JS artifacts.** When
`policy.anti_instrumentation` is set, `build` appends `guard::JS_GUARD_SOURCE` to
JS/Electron files (single-file and tree); it is behavior-safe (records
`globalThis.__DIREA_GUARD__`, never throws/blocks). Native embedding needs a
linked library and is tracked as B-027. [Evidence:
`crates/direa-core/src/guard.rs` (`JS_GUARD_SOURCE`, test
`js_guard_source_is_well_formed`); `crates/direa-cli/src/build.rs`
(`harden_text_file`/`harden_tree`), test `js_guard_is_injected_when_anti_instrumentation`]
-> Keep embedded guards behavior-safe and opt-in. (ADR-0014)

**K-027 — External adapters are discovered generically.** `build` iterates the
registry's external entries for the target kind and resolves each via
`DIREA_<ID>_ADAPTER` then its command on `PATH`; absence is a note. LLVM is a
compile-time hook (`DIREA_LLVM_BUILD`), while CLR/JVM adapters post-process.
[Evidence: `crates/direa-cli/src/build.rs` (`apply_external_adapters`,
`adapter_env_key`); test `adapter_env_key_is_derived`;
`adapters/llvm/direa-llvm`] -> New adapters only need a registry entry, a script
and a `DIREA_<ID>_ADAPTER` doc line. (ADR-0002, ADR-0009)

**K-026 — The canary gauntlet is the detector's regression test.** `canary::gauntlet`
runs `detect_injection` over a fixed adversarial corpus (must all be caught) and
over our own gate/decoy payloads (must not be flagged); `die-a-rea canary-selftest`
exposes it and CI runs it. [Evidence: `crates/direa-core/src/canary.rs`
(`gauntlet`, test `gauntlet_passes_and_payloads_are_clean`); `main.rs`
`cmd_canary_selftest`] -> Extend the corpus when new injection styles appear; keep
`passed` fatal. (ADR-0006)

**K-025 — Agent-layer content lives in `direa_core::canary`.** `gate_text`,
`canary_token`, `decoy_analysis` and `detect_injection` are the single home for
the authorization gate, traceable tokens, opt-in decoy text (bounded by
`MAX_DECOY_LINES`) and the prompt-injection detector; `build` consumes them.
[Evidence: `crates/direa-core/src/canary.rs` (unit tests);
`crates/direa-cli/src/build.rs`] -> Do not inline gate/canary logic in `build`;
extend the module. (ADR-0006)

**K-024 — Mach-O fat containers and `__DWARF` need explicit handling.** `detect`
must recognise the fat magics (`0xcafebabe`/`0xcafebabf`, both byte orders) or
fat images fall through as `Unknown`; `strip_macho` then iterates slices, and
`__DWARF` segment/section data is zeroed with names scrubbed (like ELF). [Evidence:
`crates/direa-core/src/native_post.rs` (`strip_fat`, `strip_dwarf_segment`);
tests `macho_fat_slices_are_stripped`, `macho_dwarf_is_scrubbed`] -> New Mach-O
features must cover thin, fat and `__DWARF`. (ADR-0008)

**K-023 — PE resources are runtime-needed and intentionally preserved.**
`native_post` strips the PE Rich header fingerprint (build-tool correlation) but
leaves the resource directory intact — icons, manifests and version info are used
at runtime. [Evidence: `crates/direa-core/src/native_post.rs`
(`strip_pe_rich_header`); test `pe_rich_header_scrubbed`] -> Strip fingerprints,
not resources; any future resource scrub must be opt-in and behaviour-tested.
(ADR-0008)

**K-022 — MSRV is 1.79; the exercised toolchain is 1.96.1.** `std::path::absolute`
(used by `build`'s output guard) stabilised in Rust 1.79, which sets the floor;
CI pins `stable`. [Evidence: `Cargo.toml` (`rust-version = "1.79"`); `rustc
1.96.1` exercised locally] -> Raise the MSRV only with a reason, and keep
`rust-version.workspace = true` on every crate. (Open)

**K-021 — The container's JS adapter needs `NODE_PATH`; the entrypoint must not
call `fail` inside `$(…)`.** `javascript-obfuscator` is installed globally in the
images, so `NODE_PATH` must point at the global `node_modules` for `direa-js` to
`require` it. Separately, a shell function that calls `exit`/`fail` inside a
command substitution only exits the subshell — locate the CLI with a status
check, not `exec "$(resolve_cli)"`. [Evidence: `docker/Dockerfile`
(`ENV NODE_PATH`), `docker/entrypoint.sh` (`resolve_cli`/`run_cli`); verified by
`docker run … dev doctor`] -> Keep `NODE_PATH` set and always check the status of
a locating function before `exec`. (ADR-0015)

**K-020 — Runtime instrumentation is detected from the environment; a clean
environment skips the axis.** `guard::detect` reads `TracerPid`, process maps and
injected-library env; `verify --with-runtime` adds the axis only when
instrumentation is found (otherwise unknown → skipped, K-004). [Evidence:
`crates/direa-core/src/guard.rs`, `crates/direa-verify/src/runtime.rs`; demo —
`FRIDA=1` adds the axis and lowers the RRS] -> Never execute or attach to the
target for this axis (ADR-0001); keep the clean case as *skip*, not *resisted*.
(ADR-0014)

**K-019 — Go build metadata is format-agnostic.** The `\xff Go buildinf:` blob
and `Go build ID: "..."` can appear in Go binaries of any container, so
`native_post` scrubs them by scanning for every format (and treats
`.go.buildinfo` / `.note.go.buildid` as ELF target sections). [Evidence:
`crates/direa-core/src/native_post.rs` (`strip_go_build_info`); tests
`go_build_info_scrubbed`, `non_go_binary_is_unaffected_by_go_scrub`] -> Strip Go
metadata by scan, not by container. (ADR-0008)

**K-018 — `build` health-checks the hardened artifact and fails on breakage.**
After each target (never in `--dry-run`) it verifies non-emptiness and that native
magic is preserved, then runs the opt-in `[verify] command` and, if enabled,
`node --check`. Any failure is a hard error. [Evidence:
`crates/direa-cli/src/build.rs` (`post_build_check`); test
`post_build_check_fails_on_bad_command`] -> The check must stay fatal; a broken
hardened artifact must never ship. (ADR-0013)

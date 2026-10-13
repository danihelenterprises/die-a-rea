# OSS evaluation harness

An **on-demand**, Docker-based harness that fetches pinned third-party release
artifacts from real open-source projects across every ecosystem `die-a-rea`
supports, runs `plan` / `build` / `verify` on them, and writes logs and the
hardened artifacts to `eval-out/` so you can read both the error handling and
the "die-a-rea'd" output.

This is a development harness. It is the **only** place in the repository that
uses network egress, and it never feeds anything back to the product;
`die-a-rea` itself stays local-only ([ADR-0021](../docs/adr/ADR-0021-oss-eval-harness-uses-network.md),
[ADR-0001](../docs/adr/ADR-0001-local-only-read-only.md)). It never executes the
target artifact (`build` is static; no `[verify]` command is configured).

## Usage

```bash
eval/run.sh                 # every case
eval/run.sh jq axios        # selected cases
eval/run.sh --list          # list case ids
```

Requires Docker. The first run builds the eval image (compiles the CLI release
binary and installs Node + `javascript-obfuscator` + `@electron/asar`). Results
land in `eval-out/` (gitignored).

## Cases (pinned)

| id | project | ecosystem | expected |
|---|---|---|---|
| `jq` | [jqlang/jq](https://github.com/jqlang/jq) 1.7.1 | C → native ELF | ok |
| `fd` | [sharkdp/fd](https://github.com/sharkdp/fd) v10.2.0 | Rust → native ELF | ok |
| `fzf` | [junegunn/fzf](https://github.com/junegunn/fzf) v0.55.0 | Go → native ELF | ok |
| `axios` | [axios/axios](https://github.com/axios/axios) v1.7.7 | JavaScript bundle | ok |
| `newtonsoft` | [JamesNK/Newtonsoft.Json](https://github.com/JamesNK/Newtonsoft.Json) 13.0.3 | .NET assembly | ok |
| `gson` | [google/gson](https://github.com/google/gson) 2.11.0 | Java `.class` | ok |
| `sqljs` | [sql-js/sql.js](https://github.com/sql-js/sql.js) 1.10.3 | WebAssembly | ok |
| `termux` | [termux/termux-app](https://github.com/termux/termux-app) v0.118.1 | Android APK | ok (detect/verify only) |
| `asar` | Electron `.asar` (harness-generated) | Electron | **known gap** (B-043) |

## Reading the output

```
eval-out/
  REPORT.md            # one table row per case + where to look
  summary.tsv          # machine-readable: id, ecosystem, detected, status, RRS, ...
  doctor.json          # what the image can do (adapters/tools)
  <id>/
    run.log            # full command output, including error[code]: lines
    report.md          # plan + verify JSON for the case
    before/            # file, sha256, bytes, strings.txt, nm.txt, sections.txt
    after/             # same, for the hardened artifact
    strings-removed.txt# set difference: recoverable strings that disappeared
    out/               # the hardened artifact — read it directly
```

- **Error handling:** `run.log` captures the CLI's `-vv` trace and any
  `error[code]: …` output. A case that fails unexpectedly is recorded with
  `status=build_failed` / `verify_failed` / `fetch_failed`, and the harness exits
  non-zero; a case with `EXPECT="fail"` is recorded as `known_gap` instead.
- **The hardened code:** read `eval-out/<id>/out/<artifact>` (for `axios` it is
  plain text with the injected notice/canaries/decoys; for binaries diff
  `before/strings.txt` against `after/strings.txt`).

## Adding a case

Drop `eval/repos/<id>.sh` defining `ECOSYSTEM`, `EXPECT`, `LICENSE`, `REPO`,
`VERSION` and a `prepare <workdir>` that downloads/builds the artifact, places it
under `$workdir/input/`, and prints its path on stdout (progress to stderr). Pin
an exact version. Set `KIND` only to override auto-detection. No `[verify]`
command is run, so untrusted artifacts are never executed.

## Known limitations

- It tests the **built-in** transforms and auto-detection; the external adapters
  (LLVM/CLR/JVM, `garble`, `wasm-opt`) are bring-your-own and are not exercised
  here except the JS path (the image ships `javascript-obfuscator`).
- `asar` is expected to fail today (single-file `.asar` read as UTF-8, B-043).
- Windows (PE) and macOS (Mach-O) artifacts are not fetched; the native path is
  covered via ELF.
- The first run surfaced two detection gaps: `fzf` (Go) and `Newtonsoft.Json`
  (.NET) are classified from the 4 KiB sniff window as native ELF/PE because
  their format markers sit later in the file (B-053); and the JS kind serializes
  as `java-script` (B-054). See `BACKLOG.md` (B-042…B-054) for the capability
  gaps this harness surfaces.

# Docker toolchain

Run and build `die-a-rea` without installing Rust, Node or the adapters locally.
Two images are defined by [`Dockerfile`](Dockerfile):

| Target | Contents | Use |
|---|---|---|
| `runtime` (default) | the `die-a-rea` CLI, Node 22, `javascript-obfuscator`, the `direa-js` adapter | hardening a project |
| `dev` | the above **plus** the Rust toolchain (cargo, rustfmt, clippy) | building, testing, contributing |

Both images use [`entrypoint.sh`](entrypoint.sh), which fails fast with a clear
message and never continues past a broken step.

## Build

From the repository root:

```bash
docker build -f docker/Dockerfile --target runtime -t die-a-rea:runtime .
docker build -f docker/Dockerfile --target dev     -t die-a-rea:dev .
```

## Use

Mount your project at `/work` and pass relative paths:

```bash
# check what the image can do
docker run --rm die-a-rea:runtime doctor

# harden a project (source is never modified; output goes to dist-hardened/)
docker run --rm -v "$PWD":/work die-a-rea:runtime plan
docker run --rm -v "$PWD":/work die-a-rea:runtime build --out dist-hardened
docker run --rm -v "$PWD":/work die-a-rea:runtime verify \
    --original dist/app.js --hardened dist-hardened/dist/app.js
```

The `dev` image can build and test the tool itself:

```bash
docker run --rm -v "$PWD":/work die-a-rea:dev test
docker run --rm -v "$PWD":/work die-a-rea:dev lint
docker run --rm -it -v "$PWD":/work die-a-rea:dev shell
```

`docker/compose.yml` wraps the runtime image for convenience:

```bash
docker compose -f docker/compose.yml run --rm die-a-rea doctor
```

## Error handling

`entrypoint.sh` uses `set -Eeuo pipefail` and an `ERR` trap; failures print
`die-a-rea: error: … (exit N) at line M` and exit non-zero. `doctor` reports
missing tools and exits non-zero if a required one (the CLI) is absent. Unknown
subcommands are handed to the CLI so it prints its own usage rather than being
silently ignored.

## Notes

- The JS adapter needs `javascript-obfuscator`, installed globally in both
  images; `NODE_PATH` points at the global `node_modules` so the adapter can
  `require` it (see K-021).
- `go`/`garble` are not installed; deep Go obfuscation remains bring-your-own
  (the built-in Go build-info scrub needs no toolchain).
- The runtime image runs as root by default; add `--user "$(id -u):$(id -g)"` if
  your project requires it.

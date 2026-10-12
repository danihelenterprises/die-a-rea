# Contributing to die-a-rea

Thanks for helping. `die-a-rea` raises the cost of automated / LLM-driven reverse
engineering and measures the result. Two ground rules:

- **Measurement, not promises.** If you add a capability, add the probe that
  proves it. Numbers over marketing.
- **Protect only what you own.** See [`docs/ETHICS.md`](docs/ETHICS.md).

## Read this first

The project keeps a "memory" so humans and agents can work with minimal
supervision. Before changing anything, read, in order:
[`AGENTS.md`](AGENTS.md) (binding rules) → [`STATE.md`](STATE.md) (work order) →
[`docs/README.md`](docs/README.md) (authority map) → [`BACKLOG.md`](BACKLOG.md) →
[`KNOWLEDGE.md`](KNOWLEDGE.md).

## Set up

Rust (MSRV 1.79; CI pins stable):

```bash
cargo build
cargo test --all
```

Or use the container toolchain (no local install):

```bash
docker build -f docker/Dockerfile --target dev -t die-a-rea:dev .
docker run --rm -v "$PWD":/work die-a-rea:dev test
docker run --rm -v "$PWD":/work die-a-rea:dev lint
```

## Checks (all must pass)

```bash
cargo fmt --all -- --check
RUSTFLAGS="-D warnings" cargo clippy --all-targets -- -D warnings
cargo test --all
bash tools/docs-check.sh
```

## Workflow

1. Pick the next item in `STATE.md`'s ordered work list, or open a `B-NNN` in
   `BACKLOG.md` first.
2. Make the change **and its tests**.
3. Record durable knowledge as a `K-NNN` in `KNOWLEDGE.md`; record decisions as a
   new immutable `ADR-NNNN` in `docs/adr/` (never edit an existing one — supersede
   it).
4. Update affected docs (the map in `docs/README.md` says who owns what).
5. Run the checks above.
6. Commit **one work-list item at a time**:
   `<area>: <imperative summary> (<ID>)`, body ending with a `Doc impact:` line.

The tree is green at every commit.

## What to contribute

- **Adapters** (highest value): one per ecosystem, behind the subprocess protocol
  ([ADR-0002](docs/adr/ADR-0002-adapter-subprocess-protocol.md)). See
  [`adapters/js/direa-js`](adapters/js/direa-js) and
  [`docs/PIGGYBACK.md`](docs/PIGGYBACK.md) for licensing rules (never link
  copyleft or proprietary code).
- **Probes / RRS axes**: the metric is the product. Read
  [ADR-0011](docs/adr/ADR-0011-string-exposure-measures-verbatim-survival.md) and
  [`docs/RRS.md`](docs/RRS.md) before touching an axis.
- **Docs** and the Docker toolchain.

## Style

- Rust: `rustfmt` and `clippy -D warnings` are enforced. Prefer small, tested
  functions; keep the core language-agnostic.
- Do not add comments that restate the code; explain *why* when it is non-obvious.
- Never commit secrets, `target/`, or generated outputs.

## License

MIT — see [`LICENSE`](LICENSE). By contributing you agree your work is licensed
under it. Third-party tools keep their own licenses; see
[`docs/PIGGYBACK.md`](docs/PIGGYBACK.md).

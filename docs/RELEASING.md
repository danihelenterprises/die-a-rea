# Releasing

## Versioning

The workspace uses a single version (`Cargo.toml` → `[workspace.package] version`).
Keep all crates in lockstep. Follow Semantic Versioning; document changes in
[`CHANGELOG.md`](../CHANGELOG.md).

## Publish to crates.io

Internal dependencies carry both `path` and `version`, so publish in dependency
order (a dependency must exist before its dependents):

```bash
cargo publish -p direa-core
cargo publish -p direa-verify
cargo publish -p direa-cli
cargo publish -p direa-ffi
```

`cargo package -p direa-core --allow-dirty` validates packaging before publishing.
Publishing requires a crates.io token (`cargo login`); it is the maintainer's
action, not automated.

## Binary releases

Push a tag to trigger `.github/workflows/release.yml`, which builds
`die-a-rea` for Linux, macOS and Windows and attaches archives to the GitHub
Release:

```bash
git tag v0.1.0
git push origin v0.1.0
```

## C ABI

`direa-ffi` builds a `cdylib` (`libdirea_ffi`) plus `direa_ffi.h` for callers in
other languages (`direa_version`, `direa_guard_detect`, `direa_strip_native`).
It is optional; the CLI remains the full interface.

## Definition of done

Every release must be green: `cargo fmt --all -- --check`,
`RUSTFLAGS="-D warnings" cargo clippy --all-targets -- -D warnings`,
`cargo test --all`, `bash tools/docs-check.sh`.

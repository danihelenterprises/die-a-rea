#!/usr/bin/env bash
#
# die-a-rea container entrypoint.
#
# Error handling is deliberately strict: fail fast, explain clearly, and never
# continue past a broken step. Every failure names the command and the line.
set -Eeuo pipefail

prog="die-a-rea"

log() { printf '%s: %s\n' "$prog" "$*" >&2; }
fail() { printf '%s: error: %s\n' "$prog" "$*" >&2; exit 1; }
on_err() {
  local code=$?
  printf '%s: error: command failed (exit %s) at line %s\n' "$prog" "$code" "${BASH_LINENO[0]:-?}" >&2
  exit "$code"
}
trap on_err ERR

usage() {
  cat <<'PROG'
die-a-rea container

Usage: <command> [args...]

  help                         Show this help
  doctor                       Report which tools and adapters are available
  plan|init|build|verify|profiles
                               Run the die-a-rea CLI (see `die-a-rea --help`)
  test                         cargo test --all            (dev image)
  lint                         fmt check + clippy -D warnings (dev image)
  fmt                          cargo fmt --all             (dev image)
  release                      cargo build --release       (dev image)
  shell                        Start an interactive bash shell
  <anything else>              Passed straight to the die-a-rea binary

Mount your project at /work and pass relative paths, e.g.:

  docker run --rm -v "$PWD":/work die-a-rea:runtime plan
  docker run --rm -v "$PWD":/work die-a-rea:runtime build --out dist-hardened
PROG
}

need() {
  command -v "$1" >/dev/null 2>&1 \
    || fail "'$1' is not available in this image (use the 'dev' target for build/test)"
}

doctor() {
  local missing=0
  # The CLI is required in the runtime image but built on demand in dev.
  if command -v die-a-rea >/dev/null 2>&1; then
    log "ok      die-a-rea"
  elif command -v cargo >/dev/null 2>&1; then
    log "absent  die-a-rea (dev image; run 'release' to build it)"
  else
    log "MISSING die-a-rea"
    missing=1
  fi
  for tool in node npm; do
    if command -v "$tool" >/dev/null 2>&1; then
      log "ok      $tool"
    else
      log "MISSING $tool"
      missing=1
    fi
  done
  for tool in cargo go javascript-obfuscator; do
    if command -v "$tool" >/dev/null 2>&1; then
      log "ok      $tool"
    else
      log "absent  $tool (optional)"
    fi
  done
  if [ -n "${DIREA_JS_ADAPTER:-}" ] && [ -e "${DIREA_JS_ADAPTER}" ]; then
    log "ok      DIREA_JS_ADAPTER=${DIREA_JS_ADAPTER}"
  else
    log "absent  DIREA_JS_ADAPTER (JS adapter disabled)"
  fi
  return "$missing"
}

# Locate the CLI: prefer a freshly built binary (dev workflow), else the one
# installed on PATH. Returns non-zero (and prints nothing) when neither exists.
resolve_cli() {
  local built="${CARGO_TARGET_DIR:-/work/target}/release/die-a-rea"
  if [ -x "$built" ]; then
    printf '%s' "$built"
    return 0
  fi
  if command -v die-a-rea >/dev/null 2>&1; then
    printf '%s' "die-a-rea"
    return 0
  fi
  return 1
}

# Run the CLI, failing clearly if it cannot be located. Must not be called in a
# command substitution without checking the status (a subshell exit would be
# swallowed).
run_cli() {
  local cli
  if ! cli="$(resolve_cli)"; then
    fail "die-a-rea binary not found; run 'release' first (dev image) or use the runtime image"
  fi
  exec "$cli" "$@"
}

cmd="${1:-help}"
if [ "$#" -gt 0 ]; then
  shift
fi

case "$cmd" in
  help | -h | --help) usage ;;
  doctor) doctor ;;
  shell) exec bash ;;
  test)
    need cargo
    exec cargo test --all "$@"
    ;;
  lint)
    need cargo
    cargo fmt --all -- --check
    RUSTFLAGS="-D warnings" exec cargo clippy --all-targets -- -D warnings
    ;;
  fmt)
    need cargo
    exec cargo fmt --all "$@"
    ;;
  release)
    need cargo
    exec cargo build --release "$@"
    ;;
  plan | init | build | verify | profiles)
    run_cli "$cmd" "$@"
    ;;
  *)
    # Unknown command: hand it to the CLI so it prints its own usage and exits
    # non-zero. Do not guess.
    run_cli "$cmd" "$@"
    ;;
esac

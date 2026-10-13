#!/usr/bin/env bash
#
# bench/generate.sh — materialize the binary bench fixtures (WASM, Java, ELF).
#
# Text fixtures (JS, JS tree) are committed; binary ones are generated here so
# the repository stays text-only. `die-a-rea bench` skips cases whose target is
# absent, so a missing toolchain degrades gracefully.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# WASM: magic + one custom "name" section (id 0, size 8, name len 4 + "name" + "abc").
printf '\0asm\x01\0\0\0\x00\x08\x04nameabc' >"$root/wasm-module/mod.wasm"

# Java class: magic + version + constant pool with a "SourceFile" Utf8.
printf '\xca\xfe\xba\xbe\x00\x00\x00\x34\x00\x02\x01\x00\x0aSourceFile' >"$root/java-class/App.class"

# Native ELF with symbols and debug info (needs a C compiler).
if command -v cc >/dev/null 2>&1; then
  tmp="$(mktemp -d)"
  cat >"$tmp/app.c" <<'C'
#include <stdio.h>
int secret_multiplier(int x) { return x * 42 + 7; }
int main(void) { printf("%d\n", secret_multiplier(3)); return 0; }
C
  cc -g -O0 -o "$root/native-elf/app" "$tmp/app.c"
  rm -rf "$tmp"
else
  printf 'cc not found; skipping native-elf fixture\n' >&2
fi

printf 'bench fixtures ready in %s\n' "$root"

#!/usr/bin/env bash
#
# bench/generate.sh — materialize the binary bench fixtures.
#
# Text fixtures (JS, JS tree) are committed; binary ones are generated here so
# the repository stays text-only. `die-a-rea bench` skips cases whose target is
# absent, so a missing toolchain degrades gracefully.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
mkdir -p "$root/wasm-module" "$root/java-class" "$root/native-elf" \
    "$root/native-pe" "$root/native-macho" "$root/go" "$root/dotnet" \
    "$root/electron-asar" "$root/android-apk"

# WASM: magic + one custom "name" section (id 0, size 8, name len 4 + "name" + "abc").
printf '\0asm\x01\0\0\0\x00\x08\x04nameabc' >"$root/wasm-module/mod.wasm"

# Java class: magic + version + constant pool with a "SourceFile" Utf8.
printf '\xca\xfe\xba\xbe\x00\x00\x00\x34\x00\x02\x01\x00\x0aSourceFile' >"$root/java-class/App.class"

# Synthetic native fixtures (PE, Mach-O) — no toolchain needed.
if command -v python3 >/dev/null 2>&1; then
    python3 "$root/gen_fixtures.py"
else
    printf 'python3 not found; skipping native-pe/native-macho fixtures\n' >&2
fi

# Electron .asar: an opaque binary archive (build copies it; B-043/B-055).
printf 'asar-bench-fixture\x00\x01\x02\x03' >"$root/electron-asar/app.asar"

# Android APK: detected by extension; there is no built-in APK hardening (B-042).
printf 'PK\x03\x04apk-bench-fixture' >"$root/android-apk/app.apk"

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

# Go binary (needs the Go toolchain; skipped if absent or if the build fails).
if command -v go >/dev/null 2>&1; then
    tmp="$(mktemp -d)"
    cat >"$tmp/main.go" <<'GO'
package main

import "fmt"

func main() { fmt.Println(secretMultiplier(3)) }

func secretMultiplier(x int) int { return x*42 + 7 }
GO
    (cd "$tmp" && go mod init bench >/dev/null 2>&1 && go build -o "$root/go/app" .) \
        || printf 'go build failed; skipping go fixture\n' >&2
    rm -rf "$tmp"
else
    printf 'go not found; skipping go fixture\n' >&2
fi

# .NET assembly (needs the dotnet SDK; skipped if absent or if the build fails).
if command -v dotnet >/dev/null 2>&1; then
    tmp="$(mktemp -d)"
    mkdir -p "$tmp/lib"
    printf '<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><TargetFramework>net8.0</TargetFramework></PropertyGroup></Project>\n' >"$tmp/lib/lib.csproj"
    printf 'public class C { public static int F(int x) => x * 42 + 7; }\n' >"$tmp/lib/C.cs"
    (cd "$tmp/lib" && dotnet build -c Release -o "$tmp/out" >/dev/null 2>&1) \
        || printf 'dotnet build failed; skipping dotnet fixture\n' >&2
    if [ -f "$tmp/out/lib.dll" ]; then
        cp "$tmp/out/lib.dll" "$root/dotnet/App.dll"
    fi
    rm -rf "$tmp"
else
    printf 'dotnet not found; skipping dotnet fixture\n' >&2
fi

printf 'bench fixtures ready in %s\n' "$root"

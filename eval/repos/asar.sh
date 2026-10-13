# Case: Electron .asar — a tiny app packed into an asar archive. Built-in asar
# transforms are not implemented (B-055), so `build` copies the archive verbatim
# and defers to an adapter; this verifies the copy path no longer errors on a
# binary archive (B-043).
ECOSYSTEM="Electron .asar"
KIND=""
EXPECT="ok"
LICENSE="harness-generated (no third-party code)"
REPO="https://github.com/electron/asar"
VERSION="harness-generated"

prepare() {
    local work="$1"
    local app="$work/input/app-src"
    mkdir -p "$app"
    printf 'function greet(n){return "hello "+n;}\nmodule.exports={greet};\n' >"$app/index.js"
    printf '{"name":"eval-app","version":"1.0.0","main":"index.js"}\n' >"$app/package.json"
    echo "  packing app-src into app.asar" >&2
    asar pack "$app" "$work/input/app.asar" >&2
    printf '%s\n' "$work/input/app.asar"
}

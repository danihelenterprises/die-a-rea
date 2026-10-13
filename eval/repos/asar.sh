# Case: Electron .asar — a tiny app packed into an asar archive. This currently
# FAILS: a single-file `.asar` is routed through the UTF-8 text path and errors
# (B-043). EXPECT="fail" records it as a known gap instead of a harness failure.
ECOSYSTEM="Electron .asar"
KIND=""
EXPECT="fail"
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

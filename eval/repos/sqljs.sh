# Case: sql.js — SQLite compiled to WebAssembly (custom/name sections).
ECOSYSTEM="WebAssembly"
KIND=""
EXPECT="ok"
LICENSE="MIT"
REPO="https://github.com/sql-js/sql.js"
VERSION="1.10.3"

prepare() {
    local work="$1"
    local out="$work/input/sql-wasm.wasm"
    echo "  downloading sql.js $VERSION wasm module (jsdelivr)" >&2
    curl -fsSL -o "$out" "https://cdn.jsdelivr.net/npm/sql.js@1.10.3/dist/sql-wasm.wasm"
    printf '%s\n' "$out"
}

# Case: axios — a widely used JavaScript HTTP client bundle.
ECOSYSTEM="JavaScript bundle"
KIND=""
EXPECT="ok"
LICENSE="MIT"
REPO="https://github.com/axios/axios"
VERSION="v1.7.7"

prepare() {
    local work="$1"
    local out="$work/input/axios.min.js"
    echo "  downloading axios $VERSION bundle (jsdelivr)" >&2
    curl -fsSL -o "$out" "https://cdn.jsdelivr.net/npm/axios@1.7.7/dist/axios.min.js"
    # The bundle references a source map; fetch it too so source-map stripping is
    # visible. (The target is the .js file itself.)
    curl -fsSL -o "$work/input/axios.min.js.map" \
        "https://cdn.jsdelivr.net/npm/axios@1.7.7/dist/axios.min.js.map" || true
    printf '%s\n' "$out"
}

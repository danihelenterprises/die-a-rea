# Case: jq — a widely used C utility (native ELF).
# Sourced by eval/entrypoint.sh; must define the metadata below and a
# `prepare <workdir>` that places the artifact and prints its path on stdout.
ECOSYSTEM="C (autotools) -> native ELF"
KIND=""
EXPECT="ok"
LICENSE="MIT"
REPO="https://github.com/jqlang/jq"
VERSION="jq-1.7.1"

prepare() {
    local work="$1"
    local out="$work/input/jq"
    echo "  downloading jq $VERSION release binary" >&2
    curl -fsSL -o "$out" \
        "https://github.com/jqlang/jq/releases/download/jq-1.7.1/jq-linux-amd64"
    chmod +x "$out"
    printf '%s\n' "$out"
}

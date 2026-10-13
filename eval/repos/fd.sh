# Case: fd — a Rust CLI (native ELF).
ECOSYSTEM="Rust -> native ELF"
KIND=""
EXPECT="ok"
LICENSE="MIT OR Apache-2.0"
REPO="https://github.com/sharkdp/fd"
VERSION="v10.2.0"

prepare() {
    local work="$1"
    local tgz="$work/input/fd.tar.gz"
    echo "  downloading fd $VERSION release binary" >&2
    curl -fsSL -o "$tgz" \
        "https://github.com/sharkdp/fd/releases/download/v10.2.0/fd-v10.2.0-x86_64-unknown-linux-gnu.tar.gz"
    tar -xzf "$tgz" -C "$work/input"
    printf '%s\n' "$work/input/fd-v10.2.0-x86_64-unknown-linux-gnu/fd"
}

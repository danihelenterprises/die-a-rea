# Case: fzf — a Go CLI (native ELF; exercises Go build-info/ID scrubbing).
ECOSYSTEM="Go -> native ELF"
KIND=""
EXPECT="ok"
LICENSE="MIT"
REPO="https://github.com/junegunn/fzf"
VERSION="v0.55.0"

prepare() {
    local work="$1"
    local tgz="$work/input/fzf.tgz"
    echo "  downloading fzf $VERSION release binary" >&2
    curl -fsSL -o "$tgz" \
        "https://github.com/junegunn/fzf/releases/download/v0.55.0/fzf-0.55.0-linux_amd64.tar.gz"
    mkdir -p "$work/input/fzf"
    tar -xzf "$tgz" -C "$work/input/fzf"
    printf '%s\n' "$work/input/fzf/fzf"
}

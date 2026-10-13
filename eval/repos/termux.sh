# Case: Termux — an Android APK. die-a-rea detects Android and reports the plan,
# but has no built-in APK/DEX hardening (B-042); this case documents that gap.
# The APK is downloaded for testing only and is never linked or redistributed.
ECOSYSTEM="Android APK"
KIND=""
EXPECT="ok"
LICENSE="GPL-3.0-only (test artifact only; ADR-0007)"
REPO="https://github.com/termux/termux-app"
VERSION="v0.118.1"

prepare() {
    local work="$1"
    local out="$work/input/termux.apk"
    echo "  downloading Termux $VERSION APK" >&2
    curl -fsSL -o "$out" \
        "https://github.com/termux/termux-app/releases/download/v0.118.1/termux-app_v0.118.1+github-debug_arm64-v8a.apk"
    printf '%s\n' "$out"
}

# Case: Newtonsoft.Json — a popular .NET (C#) assembly.
ECOSYSTEM=".NET (C#) assembly"
KIND=""
EXPECT="ok"
LICENSE="MIT"
REPO="https://github.com/JamesNK/Newtonsoft.Json"
VERSION="13.0.3"

prepare() {
    local work="$1"
    local nupkg="$work/input/newtonsoft.nupkg"
    echo "  downloading Newtonsoft.Json $VERSION from NuGet" >&2
    curl -fsSL -o "$nupkg" \
        "https://api.nuget.org/v3-flatcontainer/newtonsoft.json/13.0.3/newtonsoft.json.13.0.3.nupkg"
    mkdir -p "$work/input/newtonsoft"
    unzip -qo "$nupkg" -d "$work/input/newtonsoft"
    local dll
    dll="$(find "$work/input/newtonsoft/lib" -name '*.dll' | sort | head -1)"
    printf '%s\n' "$dll"
}

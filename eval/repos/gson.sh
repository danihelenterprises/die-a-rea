# Case: Gson — a popular Java library; the harness targets a single .class file
# (the JVM built-in post strips debug attributes from .class files).
ECOSYSTEM="Java (Gson) .class"
KIND=""
EXPECT="ok"
LICENSE="Apache-2.0"
REPO="https://github.com/google/gson"
VERSION="2.11.0"

prepare() {
    local work="$1"
    local jar="$work/input/gson.jar"
    echo "  downloading Gson $VERSION from Maven Central" >&2
    curl -fsSL -o "$jar" \
        "https://repo1.maven.org/maven2/com/google/code/gson/gson/2.11.0/gson-2.11.0.jar"
    mkdir -p "$work/input/gson"
    unzip -qo "$jar" -d "$work/input/gson"
    printf '%s\n' "$work/input/gson/com/google/gson/Gson.class"
}

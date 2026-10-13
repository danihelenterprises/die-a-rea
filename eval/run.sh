#!/usr/bin/env bash
# eval/run.sh — host entry point for the test bowl, the Docker OSS evaluation
# harness (B-052). Prefer the top-level `./testbowl` launcher.
#
#   ./testbowl                  # run every case
#   ./testbowl jq axios         # run selected cases
#   ./testbowl --list           # list case ids
#
# Output is written to testbowl-out/ (gitignored) on the host: per-case logs,
# before/after snapshots and the hardened artifacts, plus REPORT.md.
set -Eeuo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/.." && pwd)"
image="${TESTBOWL_IMAGE:-die-a-rea-testbowl}"
out="${TESTBOWL_OUT:-$root/testbowl-out}"

if [ "${1:-}" = "--list" ]; then
    for f in "$here"/repos/*.sh; do basename "$f" .sh; done
    exit 0
fi

echo "testbowl: building image $image (context: $root)"
docker build -f "$here/Dockerfile" -t "$image" "$root"
mkdir -p "$out"
echo "testbowl: running cases into $out"
# Run as the host user so testbowl-out/ stays owned by you (no sudo to clean up).
docker run --rm --name die-a-rea-testbowl \
    --user "$(id -u):$(id -g)" -e HOME=/tmp \
    -v "$out:/testbowl-out" "$image" "$@"

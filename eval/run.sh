#!/usr/bin/env bash
# eval/run.sh — host entry point for the Docker OSS evaluation harness (B-052).
#
#   eval/run.sh                 # run every case
#   eval/run.sh jq axios        # run selected cases
#   eval/run.sh --list          # list case ids
#
# Output is written to eval-out/ (gitignored) on the host: per-case logs,
# before/after snapshots and the hardened artifacts, plus REPORT.md.
set -Eeuo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/.." && pwd)"
image="${DIREA_EVAL_IMAGE:-die-a-rea-eval}"
out="${DIREA_EVAL_OUT:-$root/eval-out}"

if [ "${1:-}" = "--list" ]; then
    for f in "$here"/repos/*.sh; do basename "$f" .sh; done
    exit 0
fi

echo "eval: building image $image (context: $root)"
docker build -f "$here/Dockerfile" -t "$image" "$root"
mkdir -p "$out"
echo "eval: running cases into $out"
docker run --rm -v "$out:/eval-out" "$image" "$@"

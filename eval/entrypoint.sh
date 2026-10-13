#!/usr/bin/env bash
# eval/entrypoint.sh — on-demand OSS evaluation harness (runs inside the eval image).
#
# For each case in /eval-repos it fetches a pinned third-party artifact, runs
# `die-a-rea plan/build/verify`, snapshots the artifact before/after, and writes
# per-case logs plus a combined REPORT.md under /eval-out.
#
# This is a *development* harness: it is the only place network access is used.
# die-a-rea itself remains local-only (ADR-0001); the harness never feeds an
# artifact back to the product over the network.
set -uo pipefail

OUT="${EVAL_OUT:-/eval-out}"
REPOS_DIR="${EVAL_REPOS:-/eval-repos}"
SUMMARY="$OUT/summary.tsv"
LOG="$OUT/harness.log"
mkdir -p "$OUT"
: > "$SUMMARY"
# Pipe-delimited (not tab): tab is IFS-whitespace, so `read` would collapse the
# empty rrs/grade fields and shift the columns.
printf 'id|ecosystem|detected|status|rrs|grade|bytes_in|bytes_out|log\n' >>"$SUMMARY"

say() { printf '%s\n' "$*" | tee -a "$LOG"; }

# ── snapshot: capture what an analyst could recover from an artifact ─────────
snapshot() {
    local p="$1" dir="$2"
    mkdir -p "$dir"
    if [ -d "$p" ]; then
        find "$p" -type f | sort >"$dir/files.txt" 2>&1 || true
        return
    fi
    file "$p" >"$dir/file.txt" 2>&1 || true
    sha256sum "$p" >"$dir/sha256.txt" 2>&1 || true
    stat -c '%s' "$p" >"$dir/bytes.txt" 2>&1 || true
    strings -n 6 "$p" 2>/dev/null | sort -u >"$dir/strings.txt" || true
    nm -C "$p" 2>/dev/null | sort -u >"$dir/nm.txt" || true
    readelf -S "$p" 2>/dev/null | sort -u >"$dir/sections.txt" || true
}

file_size() {
    local p="$1"
    if [ -d "$p" ]; then
        du -sb "$p" 2>/dev/null | cut -f1
    else
        wc -c <"$p" 2>/dev/null | tr -d ' '
    fi
}

record() {
    printf '%s|%s|%s|%s|%s|%s|%s|%s|%s\n' "$@" >>"$SUMMARY"
}

# ── one case ─────────────────────────────────────────────────────────────────
run_case() {
    local script="$1"
    local id
    id="$(basename "$script" .sh)"
    local work="$OUT/$id"
    rm -rf "$work"
    mkdir -p "$work/input"
    LOG="$work/run.log"
    : >"$LOG"

    # shellcheck disable=SC2034
    local ECOSYSTEM="?" KIND="" EXPECT="ok" LICENSE="?" REPO="?" VERSION="?"
    # shellcheck source=/dev/null
    if ! source "$script"; then
        say "$id: could not load case script"
        record "$id" "?" "" "script_error" "" "" "" "" "$LOG"
        return 1
    fi

    say "================================================================"
    say "case: $id  [$ECOSYSTEM]  expect=$EXPECT"
    say "repo: $REPO @ $VERSION  ($LICENSE)"
    say "================================================================"

    local artifact
    artifact="$(prepare "$work" 2>>"$LOG" | tail -n 1)"
    if [ -z "$artifact" ] || [ ! -e "$artifact" ]; then
        say "  fetch FAILED (see $LOG)"
        record "$id" "$ECOSYSTEM" "" "fetch_failed" "" "" "" "" "$LOG"
        return 1
    fi
    say "  fetched: $artifact ($(file_size "$artifact") bytes)"
    say "  sha256: $(sha256sum "$artifact" | cut -d' ' -f1)"
    snapshot "$artifact" "$work/before"

    local manifest="$work/die-a-rea.toml"
    {
        printf '[project]\nname = "%s"\nversion = "1.0.0"\n\n' "$id"
        printf '[policy]\nprofile = "high"\n\n'
        printf '[agent_defense]\ncanaries = ["%s-eval"]\n\n' "$id"
        printf '[randomization]\nenabled = true\n\n'
        printf '[tarpit]\nenabled = true\npadding_kb = 64\nmax_total_kb = 256\n\n'
        printf '[[targets]]\npath = "%s"\n' "$artifact"
        [ -n "$KIND" ] && printf 'kind = "%s"\n' "$KIND"
    } >"$manifest"

    local detected=""
    say "  + plan"
    if die-a-rea plan --manifest "$manifest" --json >"$work/plan.json" 2>>"$LOG"; then
        detected="$(sed -n 's/.*"kind": *"\([^"]*\)".*/\1/p' "$work/plan.json" | head -1)"
        say "    detected kind: ${detected:-?}"
    else
        say "    plan FAILED (see $LOG)"
    fi

    local status="ok" rrs="" grade=""
    say "  + build"
    if die-a-rea -vv build --manifest "$manifest" --out "$work/out" >>"$LOG" 2>&1; then
        say "    build ok"
    else
        local rc=$?
        say "    build FAILED (rc=$rc; see $LOG)"
        status="build_failed"
    fi

    local name hardened
    name="$(basename "$artifact")"
    hardened="$work/out/$name"
    if [ "$status" = "ok" ] && [ -e "$hardened" ]; then
        snapshot "$hardened" "$work/after"
        # Read-back evidence: what strings/ symbols disappeared.
        if [ -f "$work/before/strings.txt" ] && [ -f "$work/after/strings.txt" ]; then
            comm -23 "$work/before/strings.txt" "$work/after/strings.txt" \
                >"$work/strings-removed.txt" 2>/dev/null || true
            say "    recoverable strings removed: $(wc -l <"$work/strings-removed.txt")"
        fi
        say "  + verify"
        if die-a-rea verify --original "$artifact" --hardened "$hardened" --json \
            >"$work/verify.json" 2>>"$LOG"; then
            rrs="$(sed -n 's/.*"rrs": *\([0-9]*\).*/\1/p' "$work/verify.json" | head -1)"
            grade="$(sed -n 's/.*"grade": *"\([A-F]\)".*/\1/p' "$work/verify.json" | head -1)"
            say "    RRS=$rrs  grade=${grade:-?}"
        else
            say "    verify FAILED (see $LOG)"
            status="verify_failed"
        fi
    elif [ "$status" = "ok" ]; then
        say "    hardened artifact not found at $hardened"
        status="no_output"
    fi

    # Known-gap handling: an expected failure is recorded, not counted as a
    # harness failure.
    local recorded_status="$status"
    if [ "$status" != "ok" ] && [ "$EXPECT" = "fail" ]; then
        recorded_status="known_gap"
        say "  known gap (EXPECT=fail): $status"
    fi

    local b_in b_out
    b_in="$(file_size "$artifact")"
    b_out="$(file_size "$hardened" 2>/dev/null || echo 0)"
    record "$id" "$ECOSYSTEM" "$detected" "$recorded_status" \
        "${rrs:-}" "${grade:-}" "${b_in:-}" "${b_out:-}" "$LOG"

    {
        echo "# $id"
        echo
        echo "- ecosystem: $ECOSYSTEM"
        echo "- repo: $REPO @ $VERSION ($LICENSE)"
        echo "- expect: $EXPECT"
        echo "- status: $recorded_status"
        echo "- detected: ${detected:-?}"
        echo "- RRS: ${rrs:-n/a} (grade ${grade:-?})"
        echo "- bytes: ${b_in:-?} -> ${b_out:-?}"
        echo
        echo '## plan'; cat "$work/plan.json" 2>/dev/null
        echo; echo '## verify'; cat "$work/verify.json" 2>/dev/null
    } >"$work/report.md"

    if [ "$recorded_status" = "ok" ] || [ "$recorded_status" = "known_gap" ]; then
        return 0
    fi
    return 1
}

# ── report ───────────────────────────────────────────────────────────────────
write_report() {
    {
        echo "# die-a-rea OSS evaluation"
        echo
        echo "- generated: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
        echo "- die-a-rea: $(die-a-rea --version 2>/dev/null || echo '?')"
        echo
        echo "| id | ecosystem | detected | status | RRS | grade | bytes in | bytes out |"
        echo "|---|---|---|---|---|---|---|---|"
        tail -n +2 "$SUMMARY" | while IFS='|' read -r id eco det st rrs gr bi bo log; do
            printf '| %s | %s | %s | %s | %s | %s | %s | %s |\n' \
                "$id" "$eco" "$det" "$st" "${rrs:--}" "${gr:--}" "$bi" "$bo"
        done
        echo
        echo "Per-case logs and artifacts live in \`eval-out/<id>/\`:"
        echo "\`run.log\` (full command output, incl. errors), \`before/\` and"
        echo "\`after/\` snapshots (\`strings.txt\`, \`nm.txt\`, \`sections.txt\`), and"
        echo "\`out/\` (the hardened artifact, readable). \`report.md\` summarises the case."
    } >"$OUT/REPORT.md"
}

main() {
    say "die-a-rea eval: $(die-a-rea --version 2>/dev/null || echo '?')"
    die-a-rea doctor --json >"$OUT/doctor.json" 2>"$OUT/doctor.err" || true

    local cases=()
    if [ "$#" -eq 0 ] || [ "$1" = all ]; then
        while IFS= read -r f; do cases+=("$f"); done < <(ls "$REPOS_DIR"/*.sh 2>/dev/null | sort)
    else
        local id
        for id in "$@"; do cases+=("$REPOS_DIR/$id.sh"); done
    fi

    local failures=0
    local script
    for script in "${cases[@]}"; do
        if [ ! -f "$script" ]; then
            say "no such case: $script"
            failures=$((failures + 1))
            continue
        fi
        run_case "$script" || failures=$((failures + 1))
    done

    write_report
    say "report: $OUT/REPORT.md  (unexpected failures: $failures)"
    # The harness fails only on unexpected failures; known gaps do not.
    [ "$failures" -eq 0 ]
}

main "$@"

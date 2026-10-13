#!/usr/bin/env bash
# tools/docs-check.sh — enforce die-a-rea documentation invariants.
#
# Dependency-light (bash + coreutils + grep). Checks:
#   1. every tracked doc is indexed in docs/README.md (ADRs via docs/adr/README.md)
#   2. every internal link resolves to a file that exists
#   3. every referenced id (B-NNN / K-NNN / ADR-NNNN) is defined
#   4. every commit in the recent range carries a "Doc impact:" line
#   5. counters are not restated outside STATE.md
#   6. the canonical counters in STATE.md match the repository
#
# Check 6 recomputes every derivable counter and fails on drift, so the table
# cannot rot (K-045). The tests count needs cargo; when cargo is absent the check
# is skipped with a warning (the CI "docs" job has no Rust toolchain).
#
# Env: STRICT_COUNTERS=1 makes check 5 fatal (default: warn).

set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail=0
err() { printf 'FAIL: %s\n' "$*" >&2; fail=$((fail + 1)); }
warn() { printf 'WARN: %s\n' "$*" >&2; }
ok() { printf 'ok: %s\n' "$*"; }

# All tracked markdown, excluding build artifacts and VCS.
all_md() {
    find . -name '*.md' -not -path './target/*' -not -path './.git/*' |
        sed 's|^\./||' | sort
}

# ── Check 1: every doc is indexed ────────────────────────────────────────────
check_index() {
    local missing=0
    while IFS= read -r doc; do
        [ "$doc" = "docs/README.md" ] && continue
        if [[ "$doc" == docs/adr/ADR-*.md ]]; then
            grep -qF "$(basename "$doc")" docs/adr/README.md || { err "ADR not indexed in docs/adr/README.md: $doc"; missing=1; }
            continue
        fi
        grep -qF "$doc" docs/README.md || { err "doc not indexed in docs/README.md: $doc"; missing=1; }
    done < <(all_md)
    [ "$missing" -eq 0 ] && ok "all docs indexed"
}

# ── Check 2: internal links resolve ──────────────────────────────────────────
check_links() {
    local missing=0
    while IFS= read -r doc; do
        local dir; dir="$(dirname "$doc")"
        # Grab ](target) forms from inline links/images.
        while IFS= read -r target; do
            case "$target" in
                http://*|https://*|mailto:*|'#'*) continue ;;
                '') continue ;;
            esac
            local path="${target%%#*}"
            [ -z "$path" ] && continue
            case "$path" in
                /*) continue ;;
            esac
            if [ ! -e "$dir/$path" ] && [ ! -e "$path" ]; then
                err "broken link in $doc -> $target"
                missing=1
            fi
        done < <(grep -oE '\]\([^)]+\)' "$doc" | sed -E 's/^\]\(//; s/\)$//')
    done < <(all_md)
    [ "$missing" -eq 0 ] && ok "all internal links resolve"
}

# ── Check 3: referenced ids are defined ──────────────────────────────────────
check_ids() {
    local defined
    defined="$(
        {
            grep -oE '^\| B-[0-9]{3}' BACKLOG.md 2>/dev/null | grep -oE 'B-[0-9]{3}'
            grep -oE '\*\*K-[0-9]{3}' KNOWLEDGE.md 2>/dev/null | grep -oE 'K-[0-9]{3}'
            for f in docs/adr/ADR-[0-9][0-9][0-9][0-9]*.md; do
                [ -e "$f" ] || continue
                basename "$f" | grep -oE 'ADR-[0-9]{4}'
            done
        } | sort -u
    )"
    local refs missing=0
    refs="$(grep -rhoE '\b(B-[0-9]{3}|K-[0-9]{3}|ADR-[0-9]{4})\b' \
        --include='*.md' --include='*.rs' --include='*.sh' --include='*.yml' \
        . 2>/dev/null | sort -u)"
    while IFS= read -r id; do
        [ -z "$id" ] && continue
        grep -qxF "$id" <<<"$defined" || { err "referenced id is undefined: $id"; missing=1; }
    done <<<"$refs"
    [ "$missing" -eq 0 ] && ok "all referenced ids are defined"
}

# ── Check 4: commits carry a Doc impact line ─────────────────────────────────
check_commits() {
    if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
        warn "not a git repository; skipping commit 'Doc impact:' check"
        return
    fi
    local range="HEAD"
    if git rev-parse --verify -q origin/main >/dev/null 2>&1; then
        range="origin/main..HEAD"
    fi
    local missing=0 sha body
    while IFS= read -r sha; do
        [ -z "$sha" ] && continue
        body="$(git show -s --format=%B "$sha")"
        if ! grep -q 'Doc impact:' <<<"$body"; then
            err "commit $sha is missing a 'Doc impact:' line"
            missing=1
        fi
    done < <(git rev-list --max-count=50 "$range" 2>/dev/null)
    [ "$missing" -eq 0 ] && ok "commit 'Doc impact:' lines present (range: $range)"
}

# ── Check 5: counters not restated outside STATE.md ──────────────────────────
# A counter is "<number> <label>" where the label matches a canonical metric.
COUNTER_PATTERN='[0-9]+[[:space:]]+(tests|backlog items|lessons|ADRs|decision records|markdown docs|commits)\b'

# Print offending lines for a root, excluding the canonical table in STATE.md.
count_counter_hits() {
    grep -RInE "$COUNTER_PATTERN" --include='*.md' "$1" 2>/dev/null |
        grep -v '/STATE.md:' || true
}

check_counters() {
    local hits
    hits="$(count_counter_hits "$ROOT")"
    if [ -n "$hits" ]; then
        while IFS= read -r line; do
            err "counter restated outside STATE.md: $line"
        done <<<"$hits"
    else
        ok "no counters restated outside STATE.md"
    fi
}

# ── Check 6: canonical counters match the repository ─────────────────────────
# Read the first table row in STATE.md whose label matches exactly.
state_value() {
    grep -F "| $1 |" STATE.md | head -1 |
        sed -E 's/.*\|[[:space:]]*([0-9]+)[[:space:]]*\|.*/\1/'
}

expect_counter() {
    local label="$1" actual="$2"
    local stated; stated="$(state_value "$label")"
    if [ -z "$stated" ]; then
        err "STATE.md counter missing: $label"
    elif [ "$stated" != "$actual" ]; then
        err "STATE.md counter stale: $label = $stated (repo says $actual)"
    fi
}

# Backlog rows with Severity High and Status Open.
open_high_count() {
    awk -F'|' '
        /^\| B-[0-9][0-9][0-9] / {
            sev=$4; st=$6
            gsub(/[[:space:]]/, "", sev); gsub(/[[:space:]]/, "", st)
            if (sev=="High" && st=="Open") c++
        }
        END { print c+0 }
    ' BACKLOG.md 2>/dev/null
}

# Markdown docs that are (or will be) tracked: in the index plus untracked,
# non-ignored files. Falls back to the working tree outside a git checkout.
markdown_count() {
    if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
        git ls-files --cached --others --exclude-standard '*.md' | wc -l
    else
        find . -name '*.md' -not -path './target/*' -not -path './.git/*' | wc -l
    fi
}

check_counter_values() {
    if [ ! -f STATE.md ]; then
        warn "no STATE.md; skipping counter-value check"
        return
    fi
    local before=$fail
    expect_counter "Rust crates" "$(ls -d crates/*/ 2>/dev/null | wc -l | tr -d ' ')"
    expect_counter "Backlog items (\`B-NNN\`)" \
        "$(grep -oE '^\| B-[0-9]{3}' BACKLOG.md 2>/dev/null | sort -u | wc -l | tr -d ' ')"
    expect_counter "Open backlog items (High)" "$(open_high_count)"
    expect_counter "Durable lessons (\`K-NNN\`)" \
        "$(grep -oE '^\*\*K-[0-9]{3}' KNOWLEDGE.md 2>/dev/null | sort -u | wc -l | tr -d ' ')"
    expect_counter "Decision records (\`ADR-NNNN\`)" \
        "$(ls docs/adr/ADR-[0-9][0-9][0-9][0-9]*.md 2>/dev/null | wc -l | tr -d ' ')"
    expect_counter "Markdown docs (tracked)" "$(markdown_count | tr -d ' ')"
    if command -v cargo >/dev/null 2>&1; then
        local tests
        tests="$(cargo test --all -- --list 2>/dev/null | grep -c ': test$')"
        if [ "${tests:-0}" -gt 0 ] 2>/dev/null; then
            expect_counter 'Tests (`cargo test --all`)' "$tests"
        else
            warn "cargo could not enumerate tests; skipping the tests counter"
        fi
    else
        warn "cargo not found; skipping the tests counter"
    fi
    [ "$fail" -eq "$before" ] && ok "STATE.md counters match the repository"
}

# Prove the counter detector works: a clean tree passes, a planted duplicate is
# caught. Runnable in CI and locally with `tools/docs-check.sh --self-test`.
self_test() {
    local tmp
    tmp="$(mktemp -d)"
    printf '| Tests (`cargo test --all`) | 37 |\n' >"$tmp/STATE.md"
    printf 'a clean document with no counts\n' >"$tmp/clean.md"
    if [ -n "$(count_counter_hits "$tmp")" ]; then
        printf 'docs-check: self-test FAIL (clean tree was flagged)\n' >&2
        rm -rf "$tmp"
        exit 1
    fi
    printf 'The suite has 37 tests today.\n' >"$tmp/dirty.md"
    if [ -z "$(count_counter_hits "$tmp")" ]; then
        printf 'docs-check: self-test FAIL (planted counter was missed)\n' >&2
        rm -rf "$tmp"
        exit 1
    fi
    rm -rf "$tmp"
    printf 'docs-check: counter self-test PASS\n'
}

main() {
    if [ "${1:-}" = "--self-test" ]; then
        self_test
        exit 0
    fi
    echo "docs-check: $ROOT"
    check_index
    check_links
    check_ids
    check_commits
    check_counters
    check_counter_values
    if [ "$fail" -ne 0 ]; then
        echo "docs-check: FAILED ($fail)"
        exit 1
    fi
    echo "docs-check: PASS"
}

main "$@"

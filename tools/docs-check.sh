#!/usr/bin/env bash
# tools/docs-check.sh — enforce die-a-rea documentation invariants.
#
# Dependency-light (bash + coreutils + grep). Checks:
#   1. every tracked doc is indexed in docs/README.md (ADRs via docs/adr/README.md)
#   2. every internal link resolves to a file that exists
#   3. every referenced id (B-NNN / K-NNN / ADR-NNNN) is defined
#   4. every commit in the recent range carries a "Doc impact:" line
#   5. (best-effort) counters are not restated outside STATE.md
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

# ── Check 5: counters not restated outside STATE.md (best-effort) ────────────
check_counters() {
    local hits
    hits="$(grep -RInE '[0-9]+[[:space:]]+tests?\b' --include='*.md' . 2>/dev/null |
        grep -v '^\./STATE.md:' || true)"
    if [ -n "$hits" ]; then
        if [ "${STRICT_COUNTERS:-0}" = "1" ]; then
            while IFS= read -r line; do err "counter likely restated outside STATE.md: $line"; done <<<"$hits"
        else
            while IFS= read -r line; do warn "counter likely restated outside STATE.md: $line"; done <<<"$hits"
        fi
    else
        ok "no counters restated outside STATE.md"
    fi
}

main() {
    echo "docs-check: $ROOT"
    check_index
    check_links
    check_ids
    check_commits
    check_counters
    if [ "$fail" -ne 0 ]; then
        echo "docs-check: FAILED ($fail)"
        exit 1
    fi
    echo "docs-check: PASS"
}

main "$@"

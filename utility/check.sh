#!/usr/bin/env bash
# The pre-commit gate from CLAUDE.md, in one command: format, lint, test,
# and a scan of staged content for anything that looks like a secret.
# Run from the repo root (or anywhere inside it). Exits non-zero on the
# first failure.
#
#   utility/check.sh             # fmt --check + clippy + test + secret scan
#   utility/check.sh --fix       # `cargo fmt` (rewrites) instead of --check
#   utility/check.sh --unstaged  # mid-work run: skip the secret scan, say so
#
# The secret scan reads the *staged* diff, so this belongs between
# `git add` and `git commit`, not before `git add`. Running it with an
# empty index used to print a confident "nothing secret-shaped staged" —
# a skip dressed up as a pass. It now refuses instead, unless you pass
# --unstaged to say you're only checking fmt/clippy/tests.
set -euo pipefail

FIX=0
ALLOW_UNSTAGED=0
for arg in "$@"; do
    case "$arg" in
        --fix) FIX=1 ;;
        --unstaged) ALLOW_UNSTAGED=1 ;;
        *)
            echo "usage: check.sh [--fix] [--unstaged]" >&2
            exit 2
            ;;
    esac
done

cd "$(git rev-parse --show-toplevel)"

if (( FIX )); then
    echo "==> cargo fmt"
    cargo fmt
else
    echo "==> cargo fmt --check"
    cargo fmt --check
fi

echo "==> cargo clippy --all-targets -- -D warnings"
cargo clippy --all-targets -- -D warnings

echo "==> cargo test"
cargo test

echo "==> secret scan (staged diff)"
# An empty index means this step checks nothing. Say so loudly rather
# than reporting a pass for a step that never ran.
if git diff --cached --quiet; then
    if (( ALLOW_UNSTAGED )); then
        echo "    SKIPPED -- nothing staged (--unstaged)"
        echo
        echo "OK -- fmt, clippy, tests clean. Secret scan SKIPPED (nothing staged)."
        exit 0
    fi
    echo
    echo "!! nothing staged -- the secret scan had nothing to check, so this is a"
    echo "   SKIP, not a pass. The gate runs between 'git add' and 'git commit':"
    echo
    echo "       git add -A && utility/check.sh"
    echo
    echo "   Only checking fmt/clippy/tests mid-work? Re-run with --unstaged."
    exit 1
fi

# Patterns for the obvious leak shapes: private keys, provider tokens,
# and `name = "long-opaque-value"` assignments. A hit is a warning, not a
# hard failure -- eyeball it before committing.
if git diff --cached | grep -nEi \
    'BEGIN [A-Z ]*PRIVATE KEY|xox[baprs]-[0-9A-Za-z-]{10,}|sk-[A-Za-z0-9]{20,}|ghp_[A-Za-z0-9]{20,}|AKIA[0-9A-Z]{16}|(api[_-]?key|secret|token|password)["'"'"' ]*[:=]["'"'"' ]*[A-Za-z0-9/+_-]{16,}'
then
    echo
    echo "!! staged diff contains something that looks like a secret -- review before committing"
    exit 1
fi

echo
echo "OK -- fmt, clippy, tests clean; nothing secret-shaped staged"

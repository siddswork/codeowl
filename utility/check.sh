#!/usr/bin/env bash
# The pre-commit gate from CLAUDE.md, in one command: format, lint, test,
# and a scan of staged content for anything that looks like a secret.
# Run from the repo root (or anywhere inside it). Exits non-zero on the
# first failure.
#
#   utility/check.sh            # fmt --check + clippy + test + secret scan
#   utility/check.sh --fix      # `cargo fmt` (rewrites) instead of --check
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

if [[ "${1:-}" == "--fix" ]]; then
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

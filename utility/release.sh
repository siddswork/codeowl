#!/usr/bin/env bash
# Release verification — the same gate as check.sh, but in the release
# profile, plus the actual optimized binary. Slower than check.sh; run it
# before cutting a release or after touching anything perf- or
# overflow-sensitive, not on every commit.
#
#   utility/release.sh
#
# On success the binary is at target/release/codeowl. This repo's
# .mcp.json points at target/debug/codeowl (the dogfood config), so a
# fresh release build here does not change what a running session serves.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

echo "==> cargo fmt --check"
cargo fmt --check

echo "==> cargo clippy --release --all-targets -- -D warnings"
cargo clippy --release --all-targets -- -D warnings

echo "==> cargo test --release"
cargo test --release

echo "==> cargo build --release"
cargo build --release

echo
ls -lh target/release/codeowl
echo "OK -- release profile clean; binary at target/release/codeowl"

#!/usr/bin/env bash
# The C++ accuracy check: the pack against the compiler (libclang).
# See utility/cpp_accuracy/README.md. Usage: run.sh [--refresh] [--check] [repo ...]
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"
venv="$root/target/cpp-accuracy/venv"
want="libclang==18.1.1"

command -v g++ >/dev/null || { echo "g++ is needed (its header search path is the compiler's)" >&2; exit 1; }
command -v git >/dev/null || { echo "git is needed" >&2; exit 1; }

# The Python environment lives under target/, which is not committed, and is
# rebuilt on demand. libclang is pinned: a different compiler version can
# move the reference.
if [ ! -x "$venv/bin/python" ] || ! "$venv/bin/python" -c "import clang.cindex" 2>/dev/null; then
    echo "setting up the Python environment ($want) in $venv"
    python3 -m venv "$venv"
    "$venv/bin/pip" install -q "$want"
fi

exec "$venv/bin/python" -I "$here/run.py" "$@"

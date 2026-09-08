#!/usr/bin/env python3
"""
Structural sweep — prove a refactor changed nothing a caller can observe.

Every Phase 2 milestone (M12-M18) has the same validation: the *spec-affecting*
output for a real repo must be byte-identical before and after, even though the
`.codeowl/` cache format is allowed to change. This runs that check.

It drives two `codeowl` builds through the MCP read surface against a target
repo and diffs:

  - `codeowl extract`              every symbol (the `markers` field is ignored:
                                   it's expected to be empty pre-M14 and is
                                   additive)
  - get_spec_coverage              summary + the full ordered `pending` list
  - get_next_spec_task             for every feature, every rollup, `system`,
                                   and a sample of file targets
  - get_callers / get_callees      for every schema table and a fan-in sample
  - get_symbol                     a sample

One long-lived `codeowl serve` session per binary, so the graph is built once
(not once per query). The target repo's `.codeowl/` is backed up and restored.
A `--base` git ref is built in a throwaway worktree removed on exit. Nothing
here calls an LLM.

    # current working tree vs origin/master, against the pilot
    python3 utility/structural_sweep.py --repo ~/dev/startup/talentTrail

    # two explicit binaries
    python3 utility/structural_sweep.py --repo /path/to/repo \
        --base-bin /tmp/codeowl-old --head-bin ./target/debug/codeowl

Exit code 0 iff every category matched.
"""

import argparse
import atexit
import json
import os
import shutil
import subprocess
import sys
import tempfile
import threading
import time

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


# --------------------------------------------------------------------------- #
# One long-lived MCP stdio session: initialize once, then many tools/call.
# --------------------------------------------------------------------------- #
class McpSession:
    def __init__(self, binary, repo):
        self.proc = subprocess.Popen(
            [binary, "serve", repo],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            text=True, bufsize=1)
        self._err = []
        threading.Thread(
            target=lambda: self._err.extend(l.rstrip("\n") for l in self.proc.stderr),
            daemon=True).start()
        self._id = 0
        self._send({"jsonrpc": "2.0", "id": self._next(), "method": "initialize",
                    "params": {"protocolVersion": "2025-06-18", "capabilities": {},
                               "clientInfo": {"name": "structural_sweep", "version": "0"}}})
        self._recv()
        self._send({"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}})

    def _next(self):
        self._id += 1
        return self._id

    def _send(self, obj):
        self.proc.stdin.write(json.dumps(obj) + "\n")
        self.proc.stdin.flush()

    def _recv(self, timeout=180):
        deadline = time.time() + timeout
        while time.time() < deadline:
            line = self.proc.stdout.readline()
            if not line:
                raise RuntimeError("serve closed stdout\n" + "\n".join(self._err))
            line = line.strip()
            if line:
                return json.loads(line)
        raise RuntimeError("timed out waiting for a response")

    def call(self, tool, args):
        self._send({"jsonrpc": "2.0", "id": self._next(), "method": "tools/call",
                    "params": {"name": tool, "arguments": args}})
        resp = self._recv()
        if "error" in resp:
            raise RuntimeError(f"{tool}({args}) -> {resp['error']}")
        result = resp.get("result", resp)
        if "structuredContent" in result:
            return result["structuredContent"]
        if "content" in result:
            return json.loads(result["content"][0]["text"])
        return result

    def close(self):
        try:
            self.proc.stdin.close()
            self.proc.terminate()
            self.proc.wait(timeout=5)
        except Exception:
            self.proc.kill()


# --------------------------------------------------------------------------- #
# Build helpers
# --------------------------------------------------------------------------- #
def build(worktree_dir):
    subprocess.run(["cargo", "build", "--quiet"], cwd=worktree_dir, check=True)
    return os.path.join(worktree_dir, "target", "debug", "codeowl")


def binary_for_ref(ref):
    wt = tempfile.mkdtemp(prefix="codeowl-sweep-")
    subprocess.run(["git", "worktree", "add", "--quiet", "--detach", wt, ref],
                   cwd=REPO_ROOT, check=True)
    atexit.register(lambda: subprocess.run(
        ["git", "worktree", "remove", "--force", wt],
        cwd=REPO_ROOT, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL))
    print(f"  building {ref} in a worktree ...", flush=True)
    return build(wt)


# --------------------------------------------------------------------------- #
# Query plan — the same set of calls run against each binary.
# --------------------------------------------------------------------------- #
def canon(x):
    return json.dumps(x, sort_keys=True)


def normalize_symbol(s):
    """Fold post-M14 fields back to their pre-M14 form so a refactor that
    only *relabels* (M14 commit 1: SymbolKind -> Container/Callable/Value/
    Schema + a `raw` grammar tag) still compares equal. `raw` is exactly
    the old snake_case `kind` for every TS/SQL symbol, so restoring it is
    the whole normalization."""
    s.pop("markers", None)
    raw = s.pop("raw", None)
    if raw:
        s["kind"] = raw
    return s


def run_extract(binary, repo):
    out = subprocess.run([binary, "extract", repo], capture_output=True,
                         text=True, check=True).stdout
    return [normalize_symbol(s) for s in json.loads(out)]


def query_all(session, plan):
    """Run every call in `plan` (list of (label, tool, args)); return {label: result}."""
    out = {}
    for label, tool, args in plan:
        out[label] = session.call(tool, args)
    return out


def build_plan(coverage, graph):
    plan = [("coverage", "get_spec_coverage", {})]

    by_kind = {}
    for item in coverage.get("pending", []):
        by_kind.setdefault(item["kind"], []).append(item)

    targets = (
        [i["id"] for i in by_kind.get("feature", [])]
        + [i["id"] for i in by_kind.get("rollup", [])]
        + ["system"]
        + [i["id"] for i in by_kind.get("file", [])[:15]]
    )
    for t in targets:
        plan.append((f"task:{t}", "get_next_spec_task", {"target": t}))

    tables = [n["Symbol"]["id"] for n in graph.get("nodes", [])
              if "Symbol" in n
              and (n["Symbol"].get("kind") in ("table", "schema")
                   or n["Symbol"].get("raw") == "table")]
    for tid in tables:
        plan.append((f"callers:{tid}", "get_callers", {"id": tid}))

    fan_in = sorted((i for i in by_kind.get("file", [])),
                    key=lambda i: -i["fan_in"])[:8]
    for item in fan_in:
        sym = next((n["Symbol"]["id"] for n in graph.get("nodes", [])
                    if "Symbol" in n and n["Symbol"]["file"] == item["id"]
                    and n["Symbol"].get("is_exported")), None)
        if sym:
            plan.append((f"callees:{sym}", "get_callees", {"id": sym}))
            plan.append((f"symbol:{sym}", "get_symbol", {"id": sym}))
    return plan, len(targets), len(tables)


# --------------------------------------------------------------------------- #
def sweep(base_bin, head_bin, repo):
    cache = os.path.join(repo, ".codeowl")
    backup = tempfile.mkdtemp(prefix="codeowl-cache-bak-")
    had_cache = os.path.isdir(cache)
    if had_cache:
        shutil.copytree(cache, os.path.join(backup, ".codeowl"))

    def restore():
        if os.path.isdir(cache):
            shutil.rmtree(cache)
        if had_cache:
            shutil.copytree(os.path.join(backup, ".codeowl"), cache)
    atexit.register(restore)

    results = []

    def record(name, ok, detail=""):
        results.append((name, ok, detail))
        print(f"  [{'ok ' if ok else 'FAIL'}] {name}"
              + (f"  ({detail})" if detail and not ok else ""), flush=True)

    # --- extract (own subprocess, own rebuild) ---
    print("extract x2 ...", flush=True)
    restore()
    ext_b = run_extract(base_bin, repo)
    restore()
    ext_h = run_extract(head_bin, repo)
    record("codeowl extract (all symbols, markers ignored)",
           canon(ext_b) == canon(ext_h),
           f"{len(ext_b)} vs {len(ext_h)} symbols")

    # --- one serve session per binary; graph built once each ---
    print("base session (one rebuild, then all queries) ...", flush=True)
    restore()
    sb = McpSession(base_bin, repo)
    cov_b = sb.call("get_spec_coverage", {})
    graph = json.load(open(cache + "/graph")) if os.path.isfile(cache + "/graph") else {}
    plan, n_targets, n_tables = build_plan(cov_b, graph)
    res_b = query_all(sb, plan)
    sb.close()

    print(f"head session (one rebuild, then {len(plan)} queries) ...", flush=True)
    restore()
    sh = McpSession(head_bin, repo)
    res_h = query_all(sh, plan)
    sh.close()

    restore()

    # --- diff ---
    record("get_spec_coverage", canon(res_b["coverage"]) == canon(res_h["coverage"]))

    task_diff = [lbl for lbl in res_b
                 if lbl.startswith("task:")
                 and canon(res_b[lbl]) != canon(res_h[lbl])]
    record(f"get_next_spec_task ({n_targets} targets)", not task_diff,
           ", ".join(t[5:] for t in task_diff))

    call_diff = [lbl for lbl in res_b
                 if lbl.startswith("callers:")
                 and sorted(map(canon, res_b[lbl])) != sorted(map(canon, res_h[lbl]))]
    record(f"get_callers ({n_tables} tables)", not call_diff,
           ", ".join(t[8:] for t in call_diff))

    ee_diff = [lbl for lbl in res_b
               if lbl.startswith("callees:")
               and canon(res_b[lbl]) != canon(res_h[lbl])]
    record("get_callees (sample)", not ee_diff, ", ".join(t[8:] for t in ee_diff))

    sym_diff = []
    for lbl in res_b:
        if not lbl.startswith("symbol:"):
            continue
        b = normalize_symbol(dict(res_b[lbl]))
        h = normalize_symbol(dict(res_h[lbl]))
        if canon(b) != canon(h):
            sym_diff.append(lbl[7:])
    record("get_symbol (sample, markers + raw folded)", not sym_diff, ", ".join(sym_diff))

    return results


def main():
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--repo", required=True, help="target repo to sweep")
    ap.add_argument("--base", default="origin/master",
                    help="git ref to build as baseline (default origin/master)")
    ap.add_argument("--base-bin", help="prebuilt baseline binary (skips building --base)")
    ap.add_argument("--head-bin", help="head binary (default: build current tree)")
    args = ap.parse_args()

    repo = os.path.abspath(os.path.expanduser(args.repo))
    if not os.path.isdir(repo):
        sys.exit(f"no such repo: {repo}")

    print("building binaries", flush=True)
    base_bin = args.base_bin or binary_for_ref(args.base)
    head_bin = args.head_bin or build(REPO_ROOT)
    print(f"  base: {base_bin}\n  head: {head_bin}\n", flush=True)

    print(f"sweeping {repo}\n", flush=True)
    t0 = time.time()
    results = sweep(base_bin, head_bin, repo)

    print("\n" + "=" * 60)
    failed = [r for r in results if not r[1]]
    for name, ok, detail in results:
        print(f"  {'PASS' if ok else 'FAIL'}  {name}"
              + (f"\n        {detail}" if detail and not ok else ""))
    print("=" * 60)
    print(f"({time.time() - t0:.0f}s)")
    if failed:
        n = len(failed)
        sys.exit(f"\n{n} categor{'y' if n == 1 else 'ies'} diverged.")
    print("\nall categories byte-identical — the refactor is inert.")


if __name__ == "__main__":
    main()

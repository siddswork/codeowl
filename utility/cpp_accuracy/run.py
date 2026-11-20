"""Run the C++ accuracy check: the pack against the compiler.

Normally started through ``run.sh``, which prepares the Python environment.
For each repository in ``config.json``: get it at the pinned commit, get the
compiler's reference (cached), run the pack on it, compare, print.

    run.sh                      the four measurement repositories
    run.sh leveldb fmt          some
    run.sh --controls           the control repositories only (docs/cpp-pack-plan.md
                                section 7a); never part of a default run. Their
                                failures are not printed: they are never a source
                                of rules
    run.sh --refresh            rebuild the compiler reference (slow)
    run.sh --check              exit 1 when a target of the plan is missed

Writes ``target/cpp-accuracy/results.json`` (the measurement repositories) and
``results-controls.json`` (the controls), so neither run erases the other.
"""

import argparse
import hashlib
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import compare  # noqa: E402
import truth  # noqa: E402

ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
WORK = os.path.join(ROOT, "target", "cpp-accuracy")
# Test repositories live beside this one, as the roadmap's table says.
REPOS = os.environ.get(
    "CODEOWL_TEST_REPOS", os.path.abspath(os.path.join(ROOT, "..", "test-repos"))
)


def sh(cmd, cwd=None):
    return subprocess.run(cmd, cwd=cwd, check=True, capture_output=True, text=True).stdout.strip()


def checkout(name, cfg):
    """The repository at its pinned commit, cloned if absent. Returns the path
    and whether the checkout is at the pinned commit."""
    path = os.path.join(REPOS, cfg["dir"])
    if not os.path.isdir(path):
        print(f"  cloning {cfg['url']} at {cfg['commit'][:9]} into {path}")
        # Into a side name, renamed on success: an interrupted clone must not
        # be mistaken for a checkout on the next run.
        part = path + ".partial"
        if os.path.isdir(part):
            subprocess.run(["rm", "-rf", "--", part], check=True)
        os.makedirs(part)
        sh(["git", "init", "-q"], cwd=part)
        sh(["git", "remote", "add", "origin", cfg["url"]], cwd=part)
        sh(["git", "fetch", "-q", "--depth", "1", "origin", cfg["commit"]], cwd=part)
        sh(["git", "checkout", "-q", "FETCH_HEAD"], cwd=part)
        os.rename(part, path)
    head = sh(["git", "rev-parse", "HEAD"], cwd=path)
    return path, head == cfg["commit"], head


def generated_files(path, cfg):
    """Files a build would generate from templates (leveldb's port config),
    written to a side directory so the clone stays untouched. Returns extra
    compiler arguments."""
    args = []
    for out_rel, template_rel in cfg.get("generated", {}).items():
        out = os.path.join(WORK, "generated", os.path.basename(path), out_rel)
        os.makedirs(os.path.dirname(out), exist_ok=True)
        lines = []
        for line in open(os.path.join(path, template_rel), errors="replace"):
            # `#cmakedefine01 NAME` -> `#define NAME 0`: no optional feature.
            if line.startswith("#cmakedefine01 "):
                line = f"#define {line.split()[1]} 0\n"
            lines.append(line)
        open(out, "w").writelines(lines)
        args += ["-I", os.path.join(WORK, "generated", os.path.basename(path))]
    return args


def tool_versions():
    """What the compiler's reading depends on besides the repository."""
    from importlib.metadata import version

    gpp = sh(["g++", "--version"]).splitlines()[0]
    return {"g++": gpp, "libclang": version("libclang")}


# What the compiler's reading depends on; a comment or the `control` flag is
# not one of them and must not invalidate the cache.
COMPILER_FIELDS = ("std", "include_dirs", "skip", "only", "limit", "extra_args", "generated")


def reference(name, path, cfg, extra, head, refresh):
    """The compiler's reading, from the cache when nothing it depends on changed:
    the configuration, the commit actually checked out, the generated headers,
    the compiler versions, and the script itself."""
    templates = {
        t: open(os.path.join(path, t), "rb").read().hex()
        for t in cfg.get("generated", {}).values()
    }
    key = hashlib.sha256(
        json.dumps(
            [
                {k: cfg.get(k) for k in COMPILER_FIELDS},
                extra,
                head,
                templates,
                tool_versions(),
                open(os.path.join(HERE, "truth.py"), "rb").read().hex(),
            ],
            sort_keys=True,
        ).encode()
    ).hexdigest()[:16]
    cache = os.path.join(WORK, "truth", f"{name}-{key}.json")
    if os.path.exists(cache) and not refresh:
        return json.load(open(cache))
    print(f"  compiler reference for {name} (a few minutes the first time)")
    data = truth.build_truth(
        path,
        cfg["std"],
        cfg["include_dirs"],
        cfg["skip"],
        cfg["only"],
        cfg["limit"],
        cfg["extra_args"] + extra,
    )
    os.makedirs(os.path.dirname(cache), exist_ok=True)
    json.dump(data, open(cache, "w"))
    return data


def probe(path, name):
    out = os.path.join(WORK, "probe", f"{name}.tsv")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    binary = os.path.join(ROOT, "target", "release", "examples", "cpp_probe")
    with open(out, "w") as fh:
        subprocess.run([binary, path], check=True, stdout=fh)
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("repos", nargs="*")
    ap.add_argument("--refresh", action="store_true")
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--controls", action="store_true")
    opts = ap.parse_args()

    config = json.load(open(os.path.join(HERE, "config.json")))
    controls = [n for n, c in config["repos"].items() if c.get("control")]
    if opts.controls and (opts.repos or opts.check):
        ap.error("--controls runs every control and judges none: it takes no repository names and no --check")
    if opts.controls:
        names = controls
    else:
        names = opts.repos or [n for n in config["repos"] if n not in controls]
    for n in names:
        if n not in config["repos"]:
            sys.exit(f"unknown repository `{n}`; known: {', '.join(config['repos'])}")

    print("building the pack's probe")
    subprocess.run(
        ["cargo", "build", "--release", "--example", "cpp_probe", "-q"], cwd=ROOT, check=True
    )
    codeowl_head = sh(["git", "rev-parse", "--short", "HEAD"], cwd=ROOT)

    results, missed = {}, False
    for name in names:
        cfg = config["repos"][name]
        print(f"\n{name}")
        path, pinned, head = checkout(name, cfg)
        if not pinned:
            print(f"  WARNING: checkout is at {head[:9]}, not the pinned {cfg['commit'][:9]};"
                  " numbers will differ from the experiment's")
        extra = generated_files(path, cfg)
        ref = reference(name, path, cfg, extra, head, opts.refresh)
        measures = compare.compare(path, ref, probe(path, name))
        measures["commit"] = head
        measures["judged"] = {
            k: {"measured": round(v[0], 1), "target": v[1], "met": v[2]}
            for k, v in compare.judge(measures, config["targets"][cfg["class"]]).items()
        }
        measures["control"] = bool(cfg.get("control"))
        if cfg.get("control"):
            # Reported against the targets, never judged: the file must not
            # read as a failure (`met` flags) that `--check` ignores.
            measures["reported_against_targets"] = measures.pop("judged")
        else:
            missed |= not all(v["met"] for v in measures["judged"].values())
        if cfg.get("control"):
            redact(measures)
        results[name] = measures
        report(name, cfg, measures)

    os.makedirs(WORK, exist_ok=True)
    # Two files, so a run of one kind never erases the other's numbers.
    for fname, want_control in (("results.json", False), ("results-controls.json", True)):
        part = {n: m for n, m in results.items() if m["control"] == want_control}
        if part:
            out = os.path.join(WORK, fname)
            json.dump({"codeowl": codeowl_head, "repos": part}, open(out, "w"), indent=2)
            print(f"\nwrote {out}  (codeowl {codeowl_head})")
    if opts.check and missed:
        sys.exit(1)


def redact(measures):
    """Remove every example a control's result holds (any key ending in
    `_examples`, at any depth): a name is a lead to a rule, and a control is
    never a source of rules. One place, so a field added to `compare` later is
    caught by its name."""
    if isinstance(measures, dict):
        for k in [k for k in measures if k.endswith("_examples")]:
            measures[k] = []
        for v in measures.values():
            redact(v)
    elif isinstance(measures, list):
        for v in measures:
            redact(v)


def report(name, cfg, m):
    def pct(x):
        return f"{x['found']}/{x['total']} ({x['percent']:.1f}%)"

    print(f"  files in scope {m['files_in_scope']}; compiler: {m['tu_with_errors']} unit(s) with errors, "
          f"{m['compiler_seconds']} s")
    print(f"  parse: {m['parse']['files_with_errors']}/{m['parse']['files']} files partly read, "
          f"{m['parse']['error_places']} places")
    print(f"  classes {pct(m['classes'])}   enums {pct(m['enums'])}   functions {pct(m['functions'])}")
    if m["missing_by_kind"]:
        # A control's misses are counted, never named: a name is a lead to a
        # rule, and a control is never a source of rules.
        shown = "" if cfg.get("control") else f"  e.g. {m['missing_examples'][:4]}"
        print(f"  not found: {m['missing_by_kind']}{shown}")
    print(f"  includes resolved in the repository: {m['includes_resolved']}")
    print(f"  reference edges {m['reference_edges']}; file pairs covered by direct includes "
          f"{m['file_level']['direct']:.1f}%, by transitive {m['file_level']['transitive']:.1f}%")
    for label in ("direct", "transitive"):
        e = m[f"edges_{label}"]
        print(f"  edges via {label} includes: {e['found']} found, recall {e['recall']:.1f}%, "
              f"precision {e['precision']:.1f}%")
    e = m["edges_pack"]
    print(f"  edges of the pack itself: {e['found']} found, recall {e['recall']:.1f}%, "
          f"precision {e['precision']:.1f}%; {e['only_pack']} only the pack's, "
          f"{e['only_standin']} only the stand-in's")
    for k, v in (m.get("judged") or m["reported_against_targets"]).items():
        verdict = "ok" if v["met"] else "MISSED"
        if cfg.get("control"):
            verdict += " (a control: reported, not judged)"
        print(f"  target {k:<10} {v['measured']:>6.1f}% vs {v['target']:.0f}%  {verdict}")


if __name__ == "__main__":
    main()

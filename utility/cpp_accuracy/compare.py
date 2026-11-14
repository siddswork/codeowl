"""Compare what the C++ pack reads with what the compiler reads.

Inputs: the reference from ``truth.py`` and the pack's output from
``examples/cpp_probe.rs`` (see that file for the line format). Output: a dict
of measures for one repository; ``run.py`` prints and stores it.

What is measured (``experiments/exp-08-cpp-tree-sitter-or-compiler.md``):

* **Definitions found**: of the classes, enums and functions the compiler
  sees defined in the files in scope, how many the pack also reports. Matched
  by (file, kind, name), counted with multiplicity so overloads count.
* **Edges**: a file depends on a top-level symbol of another repository file.
  The pack has no edges of its own yet (includes are plan step 2), so this
  measures the *method the plan proposes*: follow ``#include`` transitively
  and link to each top-level symbol of an included file whose name is written
  in the including file. Recall is the share of the compiler's edges found;
  precision the share of the found edges the compiler also has. Precision is
  understated on template-heavy code: the compiler does not resolve names
  inside templates that are never instantiated, so some "false" edges are
  real.
"""

import collections
import os
import re


def norm(name):
    """A name as both sides write it: no spaces, no template arguments, no
    qualification (an operator keeps its symbol)."""
    name = re.sub(r"\s+", "", name)
    prev = None
    while prev != name:
        prev = name
        name = re.sub(r"<[^<>]*>", "", name)
    if "operator" in name:
        return "operator" + name.split("operator", 1)[1]
    return name.split("::")[-1]


def strip_comments_and_strings(text):
    def rep(m):
        s = m.group(0)
        return " " if s.startswith("/") else '""' if s.startswith('"') else s

    return re.sub(r'//[^\n]*|/\*.*?\*/|"(?:\\.|[^"\\\n])*"', rep, text, flags=re.S)


def read_probe(path):
    """Parse the probe's output into per-file facts (every file the probe saw)."""
    files, symbols, includes, enumerators = {}, [], collections.defaultdict(list), collections.defaultdict(list)
    with open(path, errors="replace") as fh:
        for line in fh:
            p = line.rstrip("\n").split("\t")
            tag = p[0]
            if tag == "F" and len(p) >= 4:
                files[p[1]] = {"errors": int(p[2]), "lines": int(p[3])}
            elif tag == "I" and len(p) >= 4:
                includes[p[1]].append((p[3], p[2]))
            elif tag == "N" and len(p) >= 4:
                enumerators[p[1]].append((p[2], p[3]))
            elif tag == "S" and len(p) >= 6:
                symbols.append((p[1], p[2], p[3], p[5] == "1"))
    return files, symbols, includes, enumerators


def _ratio(a, b):
    """A percentage. An empty denominator is 0, never 100: a reference that
    found nothing, or a pack that found no edges, must not pass a target."""
    return 100.0 * a / b if b else 0.0


def compare(repo, truth, probe_path):
    scope = set(truth["files"])
    all_files, symbols, includes, enumerators = read_probe(probe_path)
    # A broken reference (libclang finding no headers, nothing parsing) would
    # otherwise show up as a perfect score.
    if not truth["defs"] or not truth["edges"] or not all_files:
        raise SystemExit(
            f"the check is not valid: the reference has {len(truth['defs'])} definitions and "
            f"{len(truth['edges'])} edges, the probe saw {len(all_files)} files. "
            "Look at the compiler's diagnostics (--refresh to rebuild the reference)."
        )

    # ---- definitions ----------------------------------------------------
    def named(n):
        return "unnamed" not in n and "anonymous" not in n

    tdefs = collections.Counter(
        (f, k, norm(n)) for f, k, n, top in truth["defs"] if f in scope and named(n)
    )
    ttop = collections.Counter(
        (f, k, norm(n))
        for f, k, n, top in truth["defs"]
        if top and k != "func" and f in scope and named(n)
    )
    got = collections.Counter(
        (f, k, norm(n))
        for f, k, n, top in symbols
        if f in scope and k in ("class", "enum", "func")
    )

    def found(kind):
        total = sum(v for (f, k, n), v in tdefs.items() if k == kind)
        hit = sum(min(v, got[key]) for key, v in tdefs.items() if key[1] == kind)
        return hit, total

    out = {
        "files_in_scope": len(scope),
        "tu_with_errors": sum(1 for v in truth["tu_errors"].values() if v),
        "compiler_seconds": round(truth["seconds"]),
    }
    for kind, label in (("class", "classes"), ("enum", "enums"), ("func", "functions")):
        hit, total = found(kind)
        out[label] = {"found": hit, "total": total, "percent": _ratio(hit, total)}
    top_hit = sum(min(v, got[key]) for key, v in ttop.items())
    out["top_level_types"] = {
        "found": top_hit,
        "total": sum(ttop.values()),
        "percent": _ratio(top_hit, sum(ttop.values())),
    }
    scoped = {f: all_files[f] for f in scope if f in all_files}
    out["parse"] = {
        "files_with_errors": sum(1 for v in scoped.values() if v["errors"]),
        "error_places": sum(v["errors"] for v in scoped.values()),
        "files": len(scoped),
    }
    missing = [k for k, v in tdefs.items() if got[k] < v]
    out["missing_examples"] = [list(k) for k in missing[:12]]
    out["missing_by_kind"] = dict(collections.Counter(k[1] for k in missing))

    # ---- edges ------------------------------------------------------------
    by_name = collections.defaultdict(list)
    for f in all_files:
        by_name[os.path.basename(f)].append(f)
    all_set = set(all_files)

    def resolve(frm, path):
        base = os.path.dirname(frm)
        for c in (
            os.path.normpath(os.path.join(base, path)),
            os.path.normpath(path),
            os.path.normpath(os.path.join("include", path)),
            os.path.normpath(os.path.join("src", path)),
        ):
            if c in all_set:
                return c, "rule"
        suffix = [f for f in by_name.get(os.path.basename(path), []) if f.endswith("/" + path)]
        if len(suffix) == 1:
            return suffix[0], "suffix"
        return None, "ambiguous" if suffix else "missing"

    direct = collections.defaultdict(set)
    resolved = collections.Counter()
    for f, paths in includes.items():
        for path, form in paths:
            g, how = resolve(f, path)
            if g:
                direct[f].add(g)
            if f in scope and (form == "q" or g):
                resolved[("quoted " if form == "q" else "angle ") + how] += 1
    out["includes_resolved"] = dict(resolved)

    memo = {}

    def closure(f):
        if f in memo:
            return memo[f]
        seen, todo = set(), list(direct.get(f, ()))
        while todo:
            g = todo.pop()
            if g not in seen:
                seen.add(g)
                todo.extend(direct.get(g, ()))
        memo[f] = seen
        return seen

    top_names = collections.defaultdict(set)
    for f, k, n, top in symbols:
        if f not in scope or not top:
            continue
        if k == "func" and "::" in n and "operator" not in n:
            continue  # an out-of-class definition belongs to its class
        nn = norm(n)
        if re.fullmatch(r"[A-Za-z_]\w*", nn):
            top_names[f].add(nn)
    enum_alias = collections.defaultdict(lambda: collections.defaultdict(set))
    for f, pairs in enumerators.items():
        if f in scope:
            for owner, e in pairs:
                enum_alias[f][owner].add(e)
                top_names[f].add(owner)

    words = {}
    for f in scope:
        try:
            text = open(os.path.join(repo, f), errors="replace").read()
        except OSError:
            text = ""
        words[f] = set(re.findall(r"[A-Za-z_]\w*", strip_comments_and_strings(text)))

    tedges = {
        (a, b, n)
        for a, b, n, k in truth["edges"]
        if a in scope and b in scope and re.fullmatch(r"[A-Za-z_]\w*", n or "")
    }

    def edges_over(reach):
        found_edges = set()
        for f in scope:
            for g in reach(f):
                for n in top_names.get(g, ()):
                    if n in words[f] or (enum_alias[g][n] & words[f]):
                        found_edges.add((f, g, n))
        return found_edges

    out["reference_edges"] = len(tedges)
    for label, reach in (("direct", lambda f: direct.get(f, set())), ("transitive", closure)):
        h = edges_over(reach)
        tp = len(h & tedges)
        out[f"edges_{label}"] = {
            "found": len(h),
            "recall": _ratio(tp, len(tedges)),
            "precision": _ratio(tp, len(h)),
        }
    file_ref = {(a, b) for a, b, n in tedges}
    out["file_level"] = {
        "pairs": len(file_ref),
        "direct": _ratio(len({(f, g) for f in scope for g in direct.get(f, ())} & file_ref), len(file_ref)),
        "transitive": _ratio(len({(f, g) for f in scope for g in closure(f)} & file_ref), len(file_ref)),
    }
    return out


def judge(measures, targets):
    """Each target of the plan (section 7), met or not."""
    got = {
        "classes": measures["classes"]["percent"],
        "functions": measures["functions"]["percent"],
        "recall": measures["edges_transitive"]["recall"],
        "precision": measures["edges_transitive"]["precision"],
    }
    return {name: (got[name], want, got[name] >= want) for name, want in targets.items()}

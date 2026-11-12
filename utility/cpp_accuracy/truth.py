"""The compiler's reading of a C++ repository: the reference the C++ pack is
measured against.

Parses every file in scope as its own translation unit with libclang and
records, for one build configuration:

* ``defs``: every class, enum and function *definition* in a repository file
  (file, kind, name, whether it is at namespace scope);
* ``edges``: every ``(file, file, name, kind)`` where the first file uses a
  declaration whose top-level owner lives in the second (a call to
  ``DB::Get`` is an edge to the class ``DB``), plus the link from an
  out-of-class definition to the header that declares it.

Needs the ``libclang`` Python package (``run.sh`` installs it). Slow: a few
minutes for a 300,000-line repository, which is why ``run.py`` caches the
result.
"""

import json
import os
import re
import subprocess
import sys
import time

import clang.cindex as ci
from clang.cindex import CursorKind as K
from clang.cindex import TranslationUnit as TU

CLASS = {
    K.CLASS_DECL,
    K.STRUCT_DECL,
    K.UNION_DECL,
    K.CLASS_TEMPLATE,
    K.CLASS_TEMPLATE_PARTIAL_SPECIALIZATION,
}
FUNC = {
    K.FUNCTION_DECL,
    K.CXX_METHOD,
    K.CONSTRUCTOR,
    K.DESTRUCTOR,
    K.CONVERSION_FUNCTION,
    K.FUNCTION_TEMPLATE,
}
SCOPE = {K.NAMESPACE, K.TRANSLATION_UNIT, K.LINKAGE_SPEC, K.UNEXPOSED_DECL}
EXTENSIONS = ("cc", "cpp", "cxx", "h", "hpp", "hh")


def system_include_dirs():
    """The compiler's own header search path, so the check does not depend on
    one machine's layout."""
    out = subprocess.run(
        ["g++", "-E", "-x", "c++", "-v", "-"],
        input="",
        capture_output=True,
        text=True,
        # The banner is translated; the markers below are the English ones.
        env={**os.environ, "LC_ALL": "C", "LANGUAGE": "C"},
    ).stderr
    dirs, on = [], False
    for line in out.splitlines():
        if line.startswith("#include <...>"):
            on = True
        elif line.startswith("End of search list"):
            break
        elif on:
            dirs.append(line.strip())
    if not dirs:
        raise SystemExit("could not read g++'s header search path; libclang would find no standard headers")
    return dirs


def files_in_scope(root, skip, only, limit):
    skip, only = re.compile(skip), re.compile(only)
    found = []
    for dp, dn, fn in os.walk(root):
        dn[:] = sorted(d for d in dn if d != ".git")
        for f in sorted(fn):
            if f.rsplit(".", 1)[-1] not in EXTENSIONS:
                continue
            rel = os.path.relpath(os.path.join(dp, f), root)
            if not skip.search(rel) and only.search(rel):
                found.append(rel)
    found.sort()
    if limit and len(found) > limit:
        step = len(found) / limit
        found = [found[int(i * step)] for i in range(limit)]
    return found


def build_truth(root, std, include_dirs, skip, only, limit, extra_args, log=print):
    """Return the reference for ``root`` as a JSON-serialisable dict."""
    sys.setrecursionlimit(20000)  # the walk recurses once per syntax level
    root = os.path.realpath(root)
    args = ["-x", "c++", f"-std={std}", "-w", "-ferror-limit=0"]
    for d in include_dirs:
        args += ["-I", os.path.join(root, d)]
    for d in system_include_dirs():
        args += ["-isystem", d]
    args += extra_args

    def rel(f):
        if f is None:
            return None
        p = os.path.realpath(f.name)
        return p[len(root) + 1 :] if p.startswith(root + os.sep) else None

    def is_top(c):
        p, lp = c.semantic_parent, c.lexical_parent
        return (p is None or p.kind in SCOPE) and (lp is None or lp.kind in SCOPE)

    def owner(c):
        """The top-level declaration ``c`` belongs to."""
        for _ in range(50):
            p = c.semantic_parent
            if p is None or p.kind in SCOPE:
                return c
            c = p
        return c

    def kind_of(o):
        if o.kind == K.MACRO_DEFINITION:
            return "macro"
        if o.kind in CLASS:
            return "class"
        if o.kind == K.ENUM_DECL:
            return "enum"
        if o.kind in FUNC:
            return "func"
        return o.kind.name.lower()

    files = files_in_scope(root, skip, only, limit)
    index = ci.Index.create()
    defs, edges, tu_errors = {}, set(), {}
    t0 = time.time()

    def visit_defs(c):
        for ch in c.get_children():
            f = rel(ch.location.file)
            if f is None:
                continue
            k = ch.kind
            # A macro that stamps out several declarations puts them all at
            # the expansion site, so the name and kind are part of the key.
            at = (f, ch.location.line, ch.location.column)
            if k in CLASS and ch.is_definition() and ch.spelling:
                defs[(*at, "class", ch.spelling)] = [f, "class", ch.spelling, is_top(ch)]
            elif k == K.ENUM_DECL and ch.is_definition() and ch.spelling:
                defs[(*at, "enum", ch.spelling)] = [f, "enum", ch.spelling, is_top(ch)]
            elif k in FUNC and ch.is_definition():
                defs[(*at, "func", ch.spelling)] = [f, "func", ch.spelling, is_top(ch)]
                continue  # nothing inside a function body is a symbol
            if k in CLASS or k in (K.NAMESPACE, K.LINKAGE_SPEC, K.UNEXPOSED_DECL):
                visit_defs(ch)

    def visit_refs(c, main):
        for ch in c.get_children():
            lf = ch.location.file
            here = lf is not None and rel(lf) == main
            if not here:
                # A declaration from another file has nothing of `main` in it,
                # except that a namespace may be reopened across files.
                if ch.kind not in (K.NAMESPACE, K.LINKAGE_SPEC):
                    continue
                if lf is not None and rel(lf) is None:
                    continue
            if here and ch.kind in FUNC and ch.is_definition():
                # A definition implements a declaration in another file.
                can = ch.canonical
                o = owner(can) if can is not None else None
                g = rel(o.location.file) if o is not None else None
                if g is not None and g != main and o.spelling:
                    edges.add((main, g, o.spelling, kind_of(o)))
            r = ch.referenced
            if here and r is not None and r != ch:
                d = r.get_definition() or r
                o = owner(d)
                g = rel(o.location.file) if o is not None else None
                if g is None and d is not r:
                    o = owner(r)
                    g = rel(o.location.file) if o is not None else None
                if g is not None and g != main and o.spelling and o.kind != K.NAMESPACE:
                    edges.add((main, g, o.spelling, kind_of(o)))
            visit_refs(ch, main)

    for n, f in enumerate(files):
        try:
            tu = index.parse(
                os.path.join(root, f),
                args=args,
                options=TU.PARSE_DETAILED_PROCESSING_RECORD,
            )
        except Exception:  # noqa: BLE001 - a TU that cannot be built is counted
            tu_errors[f] = -1
            continue
        tu_errors[f] = sum(1 for d in tu.diagnostics if d.severity >= 3)
        try:
            visit_defs(tu.cursor)
            visit_refs(tu.cursor, f)
        except RecursionError:
            # A very deep expression: counted as a unit that failed, so the
            # run goes on and the report says so.
            tu_errors[f] = -1
            log(f"    recursion limit reached in {f}; counted as failed")
        if n % 25 == 0:
            log(f"    {n}/{len(files)} {f} ({time.time() - t0:.0f}s)")

    return {
        "files": files,
        "defs": list(defs.values()),
        "edges": sorted(edges),
        "tu_errors": tu_errors,
        "seconds": time.time() - t0,
    }

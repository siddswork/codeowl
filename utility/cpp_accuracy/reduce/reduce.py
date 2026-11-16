"""Shrink a C++ file to the smallest one that still loses a namespace.

    python3 reduce.py FILE [OUT]

Whole declarations are dropped level by level (by brace depth), and a
`#if` … `#endif` region goes as one unit, so what is left stays readable.
The result is written to OUT (default: reduced.h in the temp folder) and
printed. Needs `cargo build --release --example cpp_ids` first. See
`../README.md`, "Why a namespace was lost"."""
import re, sys, os, tempfile
S = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, S)
import reduce_lib as r

def logical_end(lines, i):
    j = i
    while lines[j].rstrip().endswith("\\") and j + 1 < len(lines): j += 1
    return j

def directive(t):
    m = re.match(r"#\s*(\w+)", t); return m.group(1) if m else ""

def region_end(lines, i):
    """index of the last line of the #if...#endif region starting at i (logical lines)."""
    depth = 0; k = i
    while k < len(lines):
        e = logical_end(lines, k); d = directive(lines[k].strip())
        if d in ("if", "ifdef", "ifndef"): depth += 1
        elif d == "endif":
            depth -= 1
            if depth == 0: return e
        k = e + 1
    return None

def units(lines, level):
    out = []; depth = 0; st = {"blk": False}; start = None; i = 0
    n = len(lines)
    while i < n:
        l = lines[i]; t = l.strip()
        if start is None:
            was = st["blk"]
            if not was and depth == level:
                if t.startswith("#"):
                    d = directive(t); e = logical_end(lines, i)
                    if d in ("if", "ifdef", "ifndef"):
                        end = region_end(lines, i)
                        if end is not None:
                            inner = 0; s2 = {"blk": False}
                            for q in range(i, end + 1):
                                if not lines[q].strip().startswith("#"): inner += r.delta(lines[q], s2)
                            if inner == 0:
                                out.append((i, end)); i = end + 1; continue
                        i = e + 1; continue
                    if d in ("include", "define", "undef", "pragma", "error", "warning", "line"):
                        out.append((i, e)); i = e + 1; continue
                    i = e + 1; continue
                if not t or t.startswith("//"):
                    out.append((i, i)); i += 1; continue
            if was or depth != level or r.is_anchor(l):
                if not t.startswith("#"): depth += r.delta(l, st)
                i += 1; continue
            start = i
        if not t.startswith("#"): depth += r.delta(l, st)
        code = re.sub(r"//.*", "", l).rstrip()
        if not st["blk"] and depth == level and (code.endswith(";") or code.endswith("}")):
            out.append((start, i)); start = None
        i += 1
    return out

def ddmin(lines, level, ok):
    us = units(lines, level)
    keep = [True] * len(us)
    def build(k):
        drop = set()
        for n, (s, e) in enumerate(us):
            if not k[n]: drop.update(range(s, e + 1))
        return [l for i, l in enumerate(lines) if i not in drop]
    chunk = max(1, len(us) // 2)
    while chunk >= 1:
        s = 0
        while s < len(us):
            e = min(s + chunk, len(us)); trial = keep[:]; ch = False
            for q in range(s, e):
                if trial[q]: trial[q] = False; ch = True
            if ch and ok("\n".join(build(trial)) + "\n"): keep = trial
            s = e
        chunk //= 2
    return build(keep), len(us), sum(keep)

if __name__ == "__main__":
    text = open(sys.argv[1], errors="replace").read()
    ok = lambda t: len(r.wrong(t)) > 0
    lines = text.split("\n")
    print("start: wrong", len(r.wrong(text)), "lines", len(lines))
    for rnd in range(2):
        for level in (0, 1, 2, 3, 4, 5, 6):
            before = len(lines)
            lines, n, k = ddmin(lines, level, ok)
            if before != len(lines): print(f"round {rnd} level {level}: {n} units, {k} kept; {before} -> {len(lines)}")
    out = sys.argv[2] if len(sys.argv) > 2 else os.path.join(tempfile.gettempdir(), "reduced.h")
    open(out, "w").write("\n".join(lines) + "\n")
    print("final lines", len(lines), "wrong", r.wrong("\n".join(lines) + "\n"), "written to", out)

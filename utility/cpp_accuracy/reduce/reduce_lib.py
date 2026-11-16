"""The oracle `reduce.py` asks: after blanking, does a declaration's id lack
the namespaces that the braces around it say it is in?

Needs `cargo build --release --example cpp_ids` first."""
import re, subprocess, sys, os, tempfile
S = os.path.dirname(os.path.abspath(__file__))
exec(open(S + "/chainmeasure.py").read().split("# --- command line")[0])   # strip/chains/lines_to_chain
ROOT = os.path.abspath(os.path.join(S, "..", "..", ".."))
BIN = os.path.join(ROOT, "target", "release", "examples", "cpp_ids")
TMP = os.path.join(tempfile.gettempdir(), "cpp_reduce_probe.h")

def probe(text):
    open(TMP, "w").write(text)
    out = subprocess.run([BIN, TMP], capture_output=True, text=True).stdout.splitlines()
    errs = int(out[0].split("\t")[1]); rows = [(int(a), b) for a, b in (l.split("\t", 1) for l in out[1:])]
    return errs, rows

def wrong(text):
    errs, rows = probe(text)
    ch = lines_to_chain(text)
    bad = []
    for line, name in rows:
        want = "::".join(ch.get(line, []))
        if want and not (name == want or name.startswith(want + "::")):
            bad.append((line, name, want))
    return bad

def delta(l, st):
    d = 0; i = 0; b = l
    while i < len(b):
        if st["blk"]:
            j = b.find("*/", i)
            if j < 0: return d
            st["blk"] = False; i = j + 2; continue
        c = b[i]
        if b.startswith("/*", i): st["blk"] = True; i += 2; continue
        if b.startswith("//", i): break
        if c in "\"'":
            q = c; i += 1
            while i < len(b) and b[i] != q:
                if b[i] == "\\": i += 1
                i += 1
        elif c == "{": d += 1
        elif c == "}": d -= 1
        i += 1
    return d

def is_anchor(l):
    t = l.strip()
    return (not t or t.startswith("#") or re.fullmatch(r"[A-Z][A-Z0-9_]*", t) is not None
            or re.fullmatch(r"(public|private|protected):", t) is not None
            or (t.startswith("namespace ") and t.endswith("{"))
            or (t.startswith("}") and "namespace" in t))

def units(lines, level):
    out = []; depth = 0; st = {"blk": False}; start = None
    for i, l in enumerate(lines):
        was = st["blk"]; d = delta(l, st)
        if start is None:
            if depth != level or was or is_anchor(l):
                depth += d; continue
            start = i
        depth += d
        code = re.sub(r"//.*", "", l).rstrip()
        if not st["blk"] and depth == level and (code.endswith(";") or code.endswith("}")):
            out.append((start, i)); start = None
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
            for n in range(s, e):
                if trial[n]: trial[n] = False; ch = True
            if ch and ok("\n".join(build(trial)) + "\n"): keep = trial
            s = e
        chunk //= 2
    return build(keep), len(us), sum(keep)

if __name__ == "__main__":
    path = sys.argv[1]
    text = open(path, errors="replace").read()
    ok = lambda t: len(wrong(t)) > 0
    print("start: wrong ids", len(wrong(text)), "lines", text.count("\n"))
    lines = text.split("\n")
    for level in (2, 3, 4, 5):
        before = len(lines)
        lines, n, k = ddmin(lines, level, ok)
        print(f"level {level}: {n} units, {k} kept; {before} -> {len(lines)} lines; wrong now {len(wrong(chr(10).join(lines)+chr(10)))}")
    open(S + "/reduced.h", "w").write("\n".join(lines) + "\n")

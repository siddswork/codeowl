import re, sys, collections
def strip(text):
    # comments and strings out, newlines kept; preprocessor lines blanked
    def rep(m):
        s = m.group(0)
        return re.sub(r"[^\n]", " ", s)
    text = re.sub(r"//[^\n]*|/\*.*?\*/|\"(?:\\.|[^\"\\\n])*\"|'(?:\\.|[^'\\\n])*'", rep, text, flags=re.S)
    return re.sub(r"(?m)^[ \t]*#[^\n]*(?:\\\n[^\n]*)*", lambda m: re.sub(r"[^\n]", " ", m.group(0)), text)
def chains(text):
    """line (1-based) -> textual namespace chain (list) at the start of that line."""
    text = strip(text)
    toks = [(m.start(), m.group(0)) for m in re.finditer(r"[A-Za-z_]\w*|::|[{};]", text)]
    line_of = lambda pos: text.count("\n", 0, pos) + 1
    stack = []          # frames: list of namespace parts (maybe empty)
    out = {}
    i = 0
    pending = None      # parts of a `namespace ... {` waiting for its brace
    inline = False
    lastline = 0
    parts_acc = None
    for k, (pos, t) in enumerate(toks):
        ln = line_of(pos)
        if ln != lastline:
            out[ln] = [p for fr in stack for p in fr]
            lastline = ln
        if t == "namespace":
            parts_acc = []
            j = k + 1
            inl = k > 0 and toks[k-1][1] == "inline"
            while j < len(toks) and toks[j][1] not in ("{", ";", "="):
                if toks[j][1] not in ("::", "inline") and re.match(r"[A-Za-z_]", toks[j][1]):
                    parts_acc.append(toks[j][1])
                j += 1
            if j < len(toks) and toks[j][1] == "{":
                pending = [] if inl else parts_acc
            else:
                pending = None
        elif t == "{":
            stack.append(pending if pending is not None else [])
            pending = None
        elif t == "}":
            if stack: stack.pop()
    return out
def lines_to_chain(text):
    c = chains(text); n = text.count("\n") + 1; last = []; full = {}
    for ln in range(1, n + 2):
        if ln in c: last = c[ln]
        full[ln] = last
    return full
# --- command line (reduce_lib.py reads only what is above this line) ---
if len(sys.argv) < 3:
    sys.exit("usage: chainmeasure.py REPO PROBE_OUTPUT   (PROBE_OUTPUT: cpp_probe REPO > file)")
repo = sys.argv[1]; probe = sys.argv[2]
errs = {}; syms = []
for l in open(probe, errors="replace"):
    p = l.rstrip("\n").split("\t")
    if p[0] == "F": errs[p[1]] = int(p[2])
    elif p[0] == "S" and len(p) >= 6 and p[5] == "1" and p[2] != "macro": syms.append((p[1], p[3], int(p[4])))
cache = {}
tot = bad = 0; bad_in_err = 0; files_bad = set(); examples = []
for f, name, line in syms:
    if f not in cache:
        try: cache[f] = lines_to_chain(open(f"{repo}/{f}", errors="replace").read())
        except OSError: cache[f] = None
    ch = cache[f]
    if not ch: continue
    want = "::".join(ch.get(line, []))
    tot += 1
    # the extractor's id keeps the chain first: the name must start with it
    if want and not (name == want or name.startswith(want + "::")):
        bad += 1; files_bad.add(f)
        if errs.get(f, 0): bad_in_err += 1
        if len(examples) < 4: examples.append((f, name, want, errs.get(f, 0)))
print(f"top-level symbols {tot}; id lacks the textual chain: {bad} ({100*bad/max(tot,1):.1f}%) in {len(files_bad)} files; of them in files with parse errors: {bad_in_err}")
for e in examples: print("   e.g.", e)

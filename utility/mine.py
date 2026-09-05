#!/usr/bin/env python3
"""
Mine a Claude Code session transcript for CodeOwl experiment metrics.

Usage:
    python3 mine.py --run A                      # newest transcript in the target project dir
    python3 mine.py --run B --session <uuid>
    python3 mine.py --run A --file /path/to.jsonl
    python3 mine.py --list                        # show candidate transcripts, newest first

Writes runs/<RUN>/metrics.md, runs/<RUN>/files-read.md and runs/<RUN>/plan.md.
No third-party dependencies.
"""

import argparse
import datetime as dt
import json
import os
import re
import sys
from collections import Counter, OrderedDict

HERE = os.path.dirname(os.path.abspath(__file__))
PROJECTS = os.path.expanduser("~/.claude/projects")

# Claude Code slugifies the cwd it was launched from. Running from the repo root
# gives this dir; the parent-dir form is a fallback for a session started one level up.
CANDIDATE_PROJECT_DIRS = [
    "-home-sidd-dev-startup-talentTrail",
    "-home-sidd-dev-startup",
]

# Tools that pull repo content into context. This is the number CodeOwl claims to reduce.
EXPLORATION_TOOLS = {"Read", "Grep", "Glob", "NotebookRead"}
# Bash counts as exploration only when the command is actually inspecting source.
BASH_EXPLORE_RE = re.compile(
    r"\b(grep|rg|ag|find|cat|head|tail|sed|awk|ls|wc|tree|fd)\b"
)
# The codebase's own vocabulary, deliberately withheld from the prompt (see PROMPT.md).
VOCAB_RE = re.compile(r"discrepan", re.I)
SPEC_PATH_RE = re.compile(r"docs/specs", re.I)


def find_project_dir():
    for name in CANDIDATE_PROJECT_DIRS:
        p = os.path.join(PROJECTS, name)
        if os.path.isdir(p):
            return p
    sys.exit(
        "No transcript dir found. Looked for:\n  "
        + "\n  ".join(os.path.join(PROJECTS, d) for d in CANDIDATE_PROJECT_DIRS)
        + "\nRun the experiment from /home/sidd/dev/startup/talentTrail first."
    )


def transcripts(project_dir):
    out = []
    for f in os.listdir(project_dir):
        if f.endswith(".jsonl"):
            p = os.path.join(project_dir, f)
            out.append((os.path.getmtime(p), p))
    return [p for _, p in sorted(out, reverse=True)]


def load(path):
    rows = []
    with open(path, "r", errors="replace") as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            try:
                rows.append(json.loads(line))
            except json.JSONDecodeError:
                continue
    return rows


def text_of(block):
    """Flatten a tool_result / content block to searchable text."""
    if isinstance(block, str):
        return block
    if isinstance(block, list):
        return "\n".join(text_of(b) for b in block)
    if isinstance(block, dict):
        if "text" in block and isinstance(block["text"], str):
            return block["text"]
        return json.dumps(block, default=str)
    return str(block)


def result_payload(row):
    """Return (text, kind) for a tool result row."""
    r = row.get("toolUseResult")
    if r is None:
        return "", "none"
    if isinstance(r, str):
        return r, "str"
    if isinstance(r, dict):
        if r.get("type") == "text" and isinstance(r.get("file"), dict):
            return r["file"].get("content", "") or "", "file"
        if "stdout" in r or "stderr" in r:
            return (r.get("stdout") or "") + (r.get("stderr") or ""), "bash"
        return json.dumps(r, default=str), "other"
    return text_of(r), "other"


def analyse(rows):
    m = {
        "tool_calls": Counter(),
        "explore_calls": 0,
        "explore_bytes": 0,
        "explore_lines": 0,
        "files_read": OrderedDict(),   # path -> [reads, bytes, lines]
        "bash_explore_cmds": [],
        "in_tokens": 0,
        "cache_create": 0,
        "cache_read": 0,
        "out_tokens": 0,
        "assistant_turns": 0,
        "sidechain_turns": 0,
        "subagents": 0,
        "spec_reads": 0,
        "first_vocab_at": None,
        "vocab_source": None,
        "t_start": None,
        "t_end": None,
        "final_text": "",
        "session_ids": set(),
        "cwds": set(),
    }

    # tool_use id -> (name, input) so a result row can be attributed to its call
    calls = {}
    order = 0

    for row in rows:
        ts = row.get("timestamp")
        if ts:
            m["t_start"] = m["t_start"] or ts
            m["t_end"] = ts
        if row.get("sessionId"):
            m["session_ids"].add(row["sessionId"])
        if row.get("cwd"):
            m["cwds"].add(row["cwd"])

        typ = row.get("type")

        if typ == "assistant":
            msg = row.get("message", {})
            if row.get("isSidechain"):
                m["sidechain_turns"] += 1
            else:
                m["assistant_turns"] += 1

            u = msg.get("usage") or {}
            m["in_tokens"] += u.get("input_tokens", 0) or 0
            m["cache_create"] += u.get("cache_creation_input_tokens", 0) or 0
            m["cache_read"] += u.get("cache_read_input_tokens", 0) or 0
            m["out_tokens"] += u.get("output_tokens", 0) or 0

            for b in msg.get("content", []) or []:
                if not isinstance(b, dict):
                    continue
                if b.get("type") == "text":
                    m["final_text"] = b.get("text", "") or m["final_text"]
                elif b.get("type") == "tool_use":
                    name = b.get("name", "?")
                    inp = b.get("input", {}) or {}
                    m["tool_calls"][name] += 1
                    if name == "Agent":
                        m["subagents"] += 1
                    calls[b.get("id")] = (name, inp)

                    order += 1
                    blob = json.dumps(inp, default=str)
                    if SPEC_PATH_RE.search(blob):
                        m["spec_reads"] += 1
                    if m["first_vocab_at"] is None and VOCAB_RE.search(blob):
                        m["first_vocab_at"] = order
                        m["vocab_source"] = f"{name} input (call #{order})"

        elif typ == "user" and "toolUseResult" in row:
            # attribute to the originating tool_use
            name, inp = "?", {}
            for blk in row.get("message", {}).get("content", []) or []:
                if isinstance(blk, dict) and blk.get("type") == "tool_result":
                    name, inp = calls.get(blk.get("tool_use_id"), ("?", {}))
                    break

            payload, kind = result_payload(row)
            nbytes = len(payload)
            nlines = payload.count("\n") + (1 if payload else 0)

            is_explore = name in EXPLORATION_TOOLS or (
                name == "Bash" and BASH_EXPLORE_RE.search(inp.get("command", "") or "")
            )
            if is_explore:
                m["explore_calls"] += 1
                m["explore_bytes"] += nbytes
                m["explore_lines"] += nlines
                if name == "Bash":
                    m["bash_explore_cmds"].append(inp.get("command", "")[:160])
                if name == "Bash":
                    # key on the command itself, so each probe is its own row of evidence
                    path = "$ " + " ".join((inp.get("command", "") or "").split())[:120]
                else:
                    path = (
                        inp.get("file_path")
                        or inp.get("pattern")
                        or inp.get("path")
                        or f"({name})"
                    )
                rec = m["files_read"].setdefault(path, [0, 0, 0])
                rec[0] += 1
                rec[1] += nbytes
                rec[2] += nlines

            if SPEC_PATH_RE.search(payload[:4000]):
                m["spec_reads"] += 1
            if m["first_vocab_at"] is None and VOCAB_RE.search(payload):
                order += 1
                m["first_vocab_at"] = order
                m["vocab_source"] = f"{name} result (after call #{order - 1})"

    return m


def fmt_duration(a, b):
    try:
        ta = dt.datetime.fromisoformat(a.replace("Z", "+00:00"))
        tb = dt.datetime.fromisoformat(b.replace("Z", "+00:00"))
        s = int((tb - ta).total_seconds())
        return f"{s // 60}m {s % 60}s"
    except Exception:
        return "n/a"


def report(m, src, run):
    tc = m["tool_calls"]
    total_calls = sum(tc.values())
    kb = m["explore_bytes"] / 1024.0
    lines = [
        f"# Run {run} — metrics",
        "",
        f"- **Transcript:** `{src}`",
        f"- **Session id(s):** {', '.join(sorted(m['session_ids'])) or 'n/a'}",
        f"- **cwd:** {', '.join(sorted(m['cwds'])) or 'n/a'}",
        f"- **Wall clock:** {fmt_duration(m['t_start'], m['t_end'])}",
        "",
        "## Headline",
        "",
        "| Metric | Value |",
        "|---|---:|",
        f"| Exploration tool calls | **{m['explore_calls']}** |",
        f"| Source pulled into context | **{kb:,.1f} KB** ({m['explore_lines']:,} lines) |",
        f"| Distinct paths/patterns touched | **{len(m['files_read'])}** |",
        f"| Assistant turns (main) | {m['assistant_turns']} |",
        f"| Subagents spawned | {m['subagents']} (sidechain turns: {m['sidechain_turns']}) |",
        f"| Total tool calls | {total_calls} |",
        "",
        "## Token usage",
        "",
        "| Metric | Value |",
        "|---|---:|",
        f"| input_tokens | {m['in_tokens']:,} |",
        f"| cache_creation_input_tokens | {m['cache_create']:,} |",
        f"| **new content into context** (input + cache_creation) | **{m['in_tokens'] + m['cache_create']:,}** |",
        f"| cache_read_input_tokens | {m['cache_read']:,} |",
        f"| output_tokens | {m['out_tokens']:,} |",
        "",
        "> `input + cache_creation` is the honest cost metric: it counts content genuinely entering the",
        "> context. `cache_read` is re-reading what is already there and is ~10x cheaper, so a big",
        "> cache_read number is not evidence of waste.",
        "",
        "## Discovery probe",
        "",
        f"- First appearance of the withheld vocabulary (`discrepan*`): "
        + (f"**tool call #{m['first_vocab_at']}** — {m['vocab_source']}"
           if m["first_vocab_at"] else "**never appeared**"),
        f"- Reads touching `docs/specs`: **{m['spec_reads']}** "
        + ("← H1: the agent used the specs" if m["spec_reads"] else "← H1: specs NOT used (expected 0 in Run A)"),
        "",
        "## Tool call breakdown",
        "",
        "| Tool | Calls |",
        "|---|---:|",
    ]
    for name, n in tc.most_common():
        lines.append(f"| {name} | {n} |")

    if m["bash_explore_cmds"]:
        lines += ["", "## Bash commands counted as exploration", "", "```"]
        lines += m["bash_explore_cmds"]
        lines += ["```"]

    lines += [
        "",
        "## Manual scoring (fill from GROUND-TRUTH.md §G)",
        "",
        "| Metric | Value |",
        "|---|---|",
        "| Recall (MUST items found / total) | |",
        "| Precision (correct / claimed) | |",
        "| §E couplings surfaced | |",
        "| §C+§D decisions surfaced | |",
        "| **Factual errors** | |",
        "",
    ]
    return "\n".join(lines)


def files_report(m, run):
    rows = sorted(m["files_read"].items(), key=lambda kv: -kv[1][1])
    out = [
        f"# Run {run} — what the agent actually read",
        "",
        "Evidence for **Step 2** of the protocol: for each entry, ask *what fact was the agent after?*",
        "That answer — not intuition — defines what the spec for that file must contain.",
        "",
        "| Path / pattern | Reads | KB | Lines | What fact was it after? |",
        "|---|---:|---:|---:|---|",
    ]
    for path, (n, b, l) in rows:
        out.append(f"| `{path}` | {n} | {b/1024:,.1f} | {l:,} | |")
    out += ["", f"**Total:** {len(rows)} distinct paths, {m['explore_bytes']/1024:,.1f} KB, {m['explore_lines']:,} lines."]
    return "\n".join(out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", help="run label, e.g. A or B")
    ap.add_argument("--session", help="session uuid")
    ap.add_argument("--file", help="explicit .jsonl path")
    ap.add_argument("--list", action="store_true", help="list candidate transcripts")
    a = ap.parse_args()

    if a.list:
        d = find_project_dir()
        print(f"{d}\n")
        for p in transcripts(d)[:15]:
            ts = dt.datetime.fromtimestamp(os.path.getmtime(p)).strftime("%Y-%m-%d %H:%M")
            print(f"  {ts}  {os.path.getsize(p)/1024:9,.0f} KB  {os.path.basename(p)}")
        return

    if not a.run:
        sys.exit("--run is required (e.g. --run A)")

    if a.file:
        src = a.file
    else:
        d = find_project_dir()
        cands = transcripts(d)
        if a.session:
            src = next((p for p in cands if a.session in p), None)
            if not src:
                sys.exit(f"No transcript matching session {a.session} in {d}")
        else:
            if not cands:
                sys.exit(f"No transcripts in {d}")
            src = cands[0]

    rows = load(src)
    if not rows:
        sys.exit(f"Empty or unparseable transcript: {src}")
    m = analyse(rows)

    outdir = os.path.join(HERE, "runs", a.run)
    os.makedirs(outdir, exist_ok=True)
    rep = report(m, src, a.run)
    with open(os.path.join(outdir, "metrics.md"), "w") as fh:
        fh.write(rep + "\n")
    with open(os.path.join(outdir, "files-read.md"), "w") as fh:
        fh.write(files_report(m, a.run) + "\n")
    with open(os.path.join(outdir, "plan.md"), "w") as fh:
        fh.write(f"# Run {a.run} — agent's final plan\n\n_(last assistant text block; paste the full plan here if truncated)_\n\n"
                 + m["final_text"] + "\n")

    print(rep)
    print(f"\nwrote {outdir}/metrics.md, files-read.md, plan.md")


if __name__ == "__main__":
    main()

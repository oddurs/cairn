#!/usr/bin/env python3
"""Record the terminal samples the website shows.

The site argues that the program is worth trusting, so the samples on it are
captured from real runs rather than typed to look right — the same reason
ROADMAP.md is generated and the demo recording is checked in CI.

Usage: doc/samples.py [--out doc/samples.json] [--cairn target/release/cairn]
"""
import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

SGR = re.compile(r"\x1b\[([0-9;]*)m")

# The class names the stylesheet defines, mapped from the codes cairn emits.
CLASS = {
    "1": "bold", "2": "d", "31": "r", "32": "g",
    "33": "y", "34": "b", "35": "m", "36": "c", "90": "d",
}


def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def to_html(text):
    """ANSI to HTML, closing every span it opens."""
    out, pos, depth = [], 0, 0
    for m in SGR.finditer(text):
        out.append(esc(text[pos:m.start()]))
        for code in (m.group(1) or "0").split(";"):
            if code in ("", "0"):
                out.append("</span>" * depth)
                depth = 0
            elif code in CLASS:
                out.append('<span class="' + CLASS[code] + '">')
                depth += 1
        pos = m.end()
    out.append(esc(text[pos:]))
    out.append("</span>" * depth)
    return "".join(out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="doc/samples.json")
    ap.add_argument("--cairn", default="target/release/cairn")
    args = ap.parse_args()

    cairn = os.path.abspath(args.cairn)
    if not os.access(cairn, os.X_OK):
        sys.exit("no cairn binary at %s - run `cargo build --release` first" % cairn)
    out_path = os.path.abspath(args.out)

    work = tempfile.mkdtemp(prefix="cairn-samples-")
    env = dict(os.environ, CAIRN_USER="you", COLUMNS="96")
    # Pin the clock. Items record the date they were created, so without this a
    # recording differs from the committed one every day.
    env["SOURCE_DATE_EPOCH"] = "1788566400"  # 2026-09-05
    env.pop("NO_COLOR", None)
    env["PATH"] = os.path.dirname(cairn) + os.pathsep + env.get("PATH", "")

    def run(command, must_pass=True):
        real = command.replace("cairn ", cairn + " ", 1) + " --color always"
        p = subprocess.run(real, shell=True, cwd=work, env=env,
                           capture_output=True, text=True)
        if must_pass and p.returncode != 0:
            sys.exit("sample command failed: %s\n%s%s" % (command, p.stdout, p.stderr))
        return (p.stdout + p.stderr).rstrip("\n")

    # A project with enough shape to be worth looking at.
    setup = [
        'cairn init --name Nimbus --bare',
        'cairn milestone add v0.2 --title Hardening --due 2027-02-01',
        'cairn new "Support OAuth login" -t feature -m v0.1 --set priority=p0 -q',
        'cairn new "Rate-limit the public API" -t feature -m v0.2 --set priority=p1 -d 1 -q',
        'cairn new "Board shears on narrow terminals" -t bug -m v0.1 --set priority=p1 -q',
        'cairn new "Document the export format" -t docs -m v0.2 --set priority=p2 -q',
        'cairn set 1 status=doing -q',
        'cairn set 3 status=planned -q',
    ]
    for line in setup:
        run(line)

    samples = {
        "next": {"cmd": "cairn next", "text": run("cairn next")},
        "board": {"cmd": "cairn board", "text": run("cairn board")},
        "roadmap": {"cmd": "cairn roadmap", "text": run("cairn roadmap")},
        "claim": {"cmd": "cairn claim --next", "text": run("cairn claim --next")},
    }

    # The file an item actually is, straight off disk.
    item = os.path.join(work, "cairn", "items", "0001-support-oauth-login.md")
    with open(item, encoding="utf-8") as f:
        samples["item"] = {
            "path": "cairn/items/0001-support-oauth-login.md",
            "html": esc(f.read().rstrip("\n")),
        }

    # A real MCP exchange, because the agent story is the differentiator and
    # "the tool teaches the model" is only convincing if the reply is genuine.
    request = json.dumps({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": "update_item",
                   "arguments": {"id": 1, "fields": {"status": "in progress"}}},
    })
    proc = subprocess.run([cairn, "mcp"], input=request + "\n", cwd=work,
                          env=env, capture_output=True, text=True)
    reply = json.loads(proc.stdout.strip().splitlines()[-1])
    detail = reply["result"]["content"][0]["text"]
    call = 'update_item {"id": 1, "fields": {"status": "in progress"}}'
    samples["mcp"] = {
        "cmd": "cairn mcp",
        "html": (
            '<span class="d">' + esc(call) + "</span>\n\n"
            + '<span class="r">' + esc(detail.splitlines()[0]) + "</span>\n"
            + '<span class="d">' + esc("\n".join(detail.splitlines()[1:])) + "</span>"
        ),
    }

    # A failure, because the errors are half the argument.
    with open(os.path.join(work, "cairn", "items", "0009-bad.md"), "w") as f:
        f.write("---\nid: 9\ntitle: Bad\nstatus: nope\n---\n")
    samples["check"] = {"cmd": "cairn check", "text": run("cairn check", must_pass=False)}

    for s in samples.values():
        if "text" in s:
            s["html"] = to_html(s.pop("text"))

    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(samples, f, indent=2, ensure_ascii=False)
        f.write("\n")
    shutil.rmtree(work, ignore_errors=True)
    print("wrote %s (%d samples)" % (os.path.relpath(out_path), len(samples)))


if __name__ == "__main__":
    main()

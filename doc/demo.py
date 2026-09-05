#!/usr/bin/env python3
"""Record the README demo.

Runs the real commands against a throwaway project and renders the captured
output — colours and all — to an SVG. Because the output is real, the demo
cannot drift from the program: regenerate it with `make demo` and any change in
behaviour shows up in the diff.

Usage: doc/demo.py [--out doc/demo.svg] [--cairn target/release/cairn]
"""
import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile

# --- the script being demonstrated ------------------------------------------
# SETUP runs but is not shown; SCRIPT is shown with its real output. Keep it to
# one screenful: this is a poster, not a manual. The narrative is the thing that
# distinguishes cairn — a dependency keeps work off the list until it is
# genuinely startable — so the script is built to show that, not to list
# features.
SETUP = [
    "cairn init --name Nimbus --bare",
    "cairn milestone add v0.2 --title Hardening --due 2027-02-01",
]

SCRIPT = [
    "cairn new 'Support OAuth login' -t feature -m v0.1 --set priority=p0",
    "cairn new 'Rate-limit the public API' -t feature -m v0.2 --set priority=p1 -d 1",
    "cairn next",
    "cairn claim --next",
    "cairn close 1",
    "cairn next",
    "cairn board",
    "cairn render",
]

# --- ANSI -------------------------------------------------------------------
SGR = re.compile(r"\x1b\[([0-9;]*)m")

PALETTE = {
    "30": "#3b4252", "31": "#e06c75", "32": "#98c379", "33": "#e5c07b",
    "34": "#61afef", "35": "#c678dd", "36": "#56b6c2", "37": "#dcdfe4",
    "90": "#6b7385",
}
FG = "#dcdfe4"
DIM = "#7f8797"
BG = "#1b1f27"
CHROME = "#12151b"
PROMPT = "#98c379"
CMD = "#dcdfe4"


def runs(line):
    """Split a line with ANSI codes into (text, colour, bold) runs."""
    out, pos, colour, bold, dim = [], 0, None, False, False
    for m in SGR.finditer(line):
        if m.start() > pos:
            out.append((line[pos:m.start()], colour, bold, dim))
        for code in (m.group(1) or "0").split(";"):
            if code in ("", "0"):
                colour, bold, dim = None, False, False
            elif code == "1":
                bold = True
            elif code == "2":
                dim = True
            elif code in PALETTE:
                colour = PALETTE[code]
        pos = m.end()
    if pos < len(line):
        out.append((line[pos:], colour, bold, dim))
    return out


def esc(s):
    return (s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;"))


def render(session, out_path):
    char_w, line_h, pad = 8.4, 20.0, 22.0
    top = 44.0
    width_chars = max((len(strip(l)) for _, l in session), default=80)
    width = max(760.0, width_chars * char_w + pad * 2)
    height = top + len(session) * line_h + pad

    parts = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0f}" height="{height:.0f}" '
        f'viewBox="0 0 {width:.0f} {height:.0f}" font-family="ui-monospace, SFMono-Regular, '
        f'Menlo, Consolas, monospace" font-size="13">',
        '<defs><clipPath id="r">'
        f'<rect x="0" y="0" width="{width:.0f}" height="{height:.0f}" rx="9"/>'
        "</clipPath></defs>",
        f'<g clip-path="url(#r)">',
        f'<rect width="{width:.0f}" height="{height:.0f}" fill="{BG}"/>',
        f'<rect width="{width:.0f}" height="30" fill="{CHROME}"/>',
        '<circle cx="19" cy="15" r="5" fill="#e06c75"/>',
        '<circle cx="37" cy="15" r="5" fill="#e5c07b"/>',
        '<circle cx="55" cy="15" r="5" fill="#98c379"/>',
        f'<text x="{width/2:.0f}" y="19" fill="{DIM}" text-anchor="middle" '
        f'font-size="11">cairn</text>',
    ]

    y = top + 4
    for kind, line in session:
        x = pad
        if kind == "cmd":
            parts.append(
                f'<text x="{x:.1f}" y="{y:.1f}" fill="{PROMPT}" font-weight="600">$</text>'
            )
            parts.append(
                f'<text x="{x + char_w * 2:.1f}" y="{y:.1f}" fill="{CMD}">{esc(line)}</text>'
            )
        else:
            spans = []
            col = 0
            for text, colour, bold, dim in runs(line):
                if not text:
                    continue
                fill = colour or (DIM if dim else FG)
                weight = ' font-weight="600"' if bold else ""
                spans.append(
                    f'<tspan x="{x + col * char_w:.1f}" fill="{fill}"{weight}>{esc(text)}</tspan>'
                )
                col += len(text)
            if spans:
                parts.append(f'<text y="{y:.1f}">' + "".join(spans) + "</text>")
        y += line_h

    parts.append("</g></svg>")
    with open(out_path, "w") as f:
        f.write("\n".join(parts) + "\n")


def strip(line):
    return SGR.sub("", line)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="doc/demo.svg")
    ap.add_argument("--cairn", default="target/release/cairn")
    args = ap.parse_args()

    cairn = os.path.abspath(args.cairn)
    if not os.access(cairn, os.X_OK):
        sys.exit(f"no cairn binary at {cairn} — run `cargo build --release` first")
    out = os.path.abspath(args.out)

    work = tempfile.mkdtemp(prefix="cairn-demo-")
    session = []
    try:
        env = dict(os.environ, CAIRN_USER="claude", COLUMNS="88")
        env.pop("NO_COLOR", None)

        for command in SETUP:
            subprocess.run(
                command.replace("cairn ", cairn + " ", 1),
                shell=True, cwd=work, env=env, capture_output=True, text=True, check=True,
            )

        for command in SCRIPT:
            real = command.replace("cairn ", cairn + " ", 1)
            session.append(("cmd", command))
            proc = subprocess.run(
                real + " --color always",
                shell=True, cwd=work, env=env,
                capture_output=True, text=True,
            )
            if proc.returncode != 0:
                # A demo that records a failure is a demo that lies about the
                # program; better to stop and fix the script.
                sys.exit(
                    f"demo command failed: {command}\n"
                    f"{(proc.stdout + proc.stderr).strip()}"
                )
            output = (proc.stdout + proc.stderr).rstrip("\n")
            if output:
                for line in output.split("\n"):
                    session.append(("out", line))
            session.append(("out", ""))
        while session and session[-1][1] == "":
            session.pop()
        render(session, out)
        print(f"wrote {os.path.relpath(out)}  ({len(session)} lines)")
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    main()

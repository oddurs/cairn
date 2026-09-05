---
id: 43
title: Generate the site's terminal samples instead of hand-colouring them
type: bug
status: done
milestone: v0.2
created: 2026-09-05
updated: 2026-09-05
priority: p1
effort: s
---

## Problem

The landing page claimed every code sample was real output. They were not —
they were hand-written HTML with colour markup typed to look like the program.

That is precisely the drift the rest of the project refuses to accept.
`ROADMAP.md` is generated, the demo recording is checked in CI, and the
specification renders from source. The one place restating the program by hand
was the page whose whole argument is that the program can be trusted.

## Proposal

Record the samples the way the demo is recorded: run the real commands, capture
the ANSI, convert it to HTML, commit the result. The site imports it and CI
fails on a diff.

While there: the board — the most distinctive thing the program prints — did
not appear on the landing page at all.

## Acceptance criteria

- [x] Every terminal on the site is captured from a real run
- [x] Regenerating is one command, and CI fails if the committed copy is stale
- [x] The board appears on the landing page
- [x] The only hand-written markup left is configuration, which is not output

## 2026-09-05

Recorded rather than hand-coloured. doc/samples.py runs the commands, converts the ANSI, and commits doc/samples.json; a Terminal component renders it and CI regenerates and diffs. Seven recordings, including the board and a genuine MCP rejection. The TOML block stays hand-written because configuration is something you write, not something the program prints — and the page now says that rather than implying otherwise.

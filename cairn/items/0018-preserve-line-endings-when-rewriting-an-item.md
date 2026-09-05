---
id: 18
title: Preserve line endings when rewriting an item
type: bug
status: done
milestone: v0.1
assignee: claude
created: 2026-09-04
updated: 2026-09-04
priority: p0
sprint: s2
effort: s
---

## Problem

Confirmed by experiment. A CRLF item file parses correctly, but after
`cairn set` the frontmatter is rewritten as LF while the body keeps its CRLFs —
producing a file with mixed line endings:

    2d 2d 2d 0a  0a 0d 0a  42 6f 64 79        ---\n \n \r\n Body

On Windows with `core.autocrlf=true` this makes every write churn the file, and
progressively mangles bodies.

## Proposal

Detect the dominant line ending per file on read and reproduce it on write. Ship
a `.gitattributes` marking item files `text eol=lf` so a repository has one
answer regardless of client configuration.

## Acceptance criteria

- [ ] A CRLF file round-trips byte-identically through `cairn set`
- [ ] An LF file is never given CRLFs
- [ ] `.gitattributes` shipped by `cairn init`
- [ ] Test runs on Windows CI, where the bug actually bites

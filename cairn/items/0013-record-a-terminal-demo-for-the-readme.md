---
id: 13
title: Record a terminal demo for the README
type: docs
status: done
milestone: v0.1
created: 2026-09-04
updated: 2026-09-04
priority: p1
effort: s
---

## Problem

cairn is a program you look at — a board, a roadmap, a ranked list of what to
do next. The README describes those in prose, which sells none of them. For a
terminal program the recording *is* the product page.

## Proposal

One asciinema cast, thirty seconds, showing the loop that matters:
`cairn next` → `cairn claim --next` → work → `cairn close` → `cairn render`,
ending on the generated ROADMAP.md. Embed as an SVG so it renders on crates.io
and in the GitHub README without JavaScript.

## Acceptance criteria

- [ ] Recording embedded at the top of the README
- [ ] Shows the agent loop, not just `list`
- [ ] Regenerable from a script, so it does not rot with the interface

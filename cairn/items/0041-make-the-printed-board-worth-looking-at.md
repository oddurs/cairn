---
id: 41
title: Make the printed board worth looking at
type: feature
status: done
milestone: v0.2
assignee: claude
created: 2026-09-05
updated: 2026-09-05
priority: p1
effort: m
---

## Problem

    backlog 1                planned 0                in progress 1            blocked 0
    ───────────────────────  ───────────────────────  ───────────────────────  ───────────────────────
    0007 An interactive bo…                           0001 Publish to crates…

Serviceable, not good. Empty columns take a full share of the width; cards carry
an id and a truncated title and nothing else; nothing marks what is blocked or
what matters; there is no sense of the whole.

This is the view most likely to be screenshotted, and it is what the README demo
shows. Adoption is this project's bottleneck, not capability, and a board that
looks considered is how a tool gets tried.

## Why this and not the interactive one

A better print keeps every property that makes the rest of this tool testable:
it pipes, it goes over ssh, it renders in a screenshot, and the test suite can
assert on it like everything else. Roughly the whole aesthetic win, none of the
new discipline. See `0007` for when the interactive version becomes justified.

## Proposal

- Give width to columns in proportion to what they hold, rather than in equal
  shares. An empty column needs its heading and nothing more.
- Put the ranking field on the card, so the board says what the list says.
- Mark blocked work, since "cannot be started" is the most useful thing to know
  at a glance and is currently invisible.
- Close with the same summary line as `next`, so the two views agree.

Still a print. No cursor, no input, no event loop.

## Acceptance criteria

- [ ] An empty column does not consume a full column's width
- [ ] Blocked items are distinguishable without reading the item
- [ ] Output is still plain text a test can assert on, and still pipes
- [ ] Narrow terminals degrade legibly rather than shearing
- [ ] The README demo is regenerated from it

## 2026-09-05

Proportional widths, the ranking field on each card, a marker for work that cannot be started, and the same summary line as `next` so the two views agree. Still a print: no cursor, no input, no event loop.

## 2026-09-05

The demo's own output caught a third thing: `milestone add` appended, so a new dated milestone landed after a trailing `later`, which then inherited its date and stopped sorting last. Dated milestones are now filed among the dated ones.

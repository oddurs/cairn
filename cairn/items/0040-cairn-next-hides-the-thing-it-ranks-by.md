---
id: 40
title: cairn next hides the thing it ranks by
type: bug
status: done
milestone: v0.2
assignee: claude
created: 2026-09-05
updated: 2026-09-05
priority: p1
effort: s
---

## Problem

`cairn next` sorts by priority and does not show it.

    ID    STATUS       MILESTONE  BLOCKED BY  TITLE
    0001  in progress  v0.1                   Publish to crates.io and Homebrew
    0007  backlog      later                  An interactive board in the terminal

The order is not arbitrary — `0001` is p0 and `0007` is p2 — but nothing on
screen says so, so the ranking looks like a guess. `cairn list` shows priority
by default; the command whose whole job is ranking does not.

There is a second piece of noise in the same view: `BLOCKED BY` is always
present and almost always empty, because blocked work is excluded by default.
A column that is empty in the common case is costing width and attention for
nothing.

## Proposal

Show the fields the schema marks `column = true`, which is how `list` already
decides, so a project that tracks something other than priority gets the same
treatment without configuration.

Show `BLOCKED BY` only when a row has blockers — which is to say under
`--blocked`, or when a dependency is dangling.

Close with a summary line, because "what should I do now" has a shape as well
as a list: how many are ready, how many are under way, how many are waiting.

## Acceptance criteria

- [ ] The sort key is visible in the output
- [ ] No always-empty columns
- [ ] A project with no priority field is unaffected
- [ ] `--json` and `--ids` unchanged; this is the human view only

## 2026-09-05

Generalised while building it: rather than special-casing `blocked by`, any column empty for every row is dropped once the rows are known. That also removed a stale `sprint` column from this project's own output, which the special case would have missed.

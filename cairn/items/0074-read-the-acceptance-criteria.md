---
id: 74
title: Read the acceptance criteria
type: feature
status: done
milestone: v0.2
created: 2026-09-06
updated: 2026-09-07
priority: p0
effort: m
sprint: s8
---

## Problem

Every item in this project has an `## Acceptance criteria

- [x] Boxes are counted from the body, wherever they are
- [ ] `check` warns on unticked criteria in a closed item, and does not fail
- [x] `criteria` and `criteria_done` are filterable and sortable
- [x] Closing over MCP reports what remains unticked
- [x] A body with no boxes produces no warnings and no noise
- [x] Nothing is written to the file; this reads what is already there

## 2026-09-07: built, with one criterion deliberately not met

The second box stays unticked on purpose, and this is the note that says why —
which is the behaviour this item exists to make possible.

`check` reports unticked criteria only when `[project] require_criteria = true`,
not unconditionally as written above. The reason emerged from running it against
this project's own backlog the moment it worked: **63 closed items, 217 criteria,
200 of them unticked.** All were written before anything read them.

That is not a cairn problem, it is what adoption looks like. Any project turning
this on mid-life gets a wall of warnings about work finished years ago, turns it
off, and from then on it is worth nothing. An unticked box is also not a schema
violation the way an undefined status is — it is a judgement about process — so
`check` is the wrong place for it by default.

The information is actionable at the moment of closing. `cairn close` and the
MCP `close_item` tool report it there, unconditionally, and `criteria_met` makes
an audit available to anyone who wants one at any time.

Two smaller changes from the proposal, both deliberate:

`criteria_met` was added, because the filter grammar compares against literals
and cannot express `criteria_done < criteria` as the proposal assumed.

`cairn next` does not report the count. The table is already at its width, and a
count belongs where somebody is looking at one item rather than scanning many.

This project has not turned `require_criteria` on. Mass-ticking two hundred boxes
retrospectively would be precisely the ritual the item warns against, and doing
it honestly is a real pass over five weeks of work rather than a command.

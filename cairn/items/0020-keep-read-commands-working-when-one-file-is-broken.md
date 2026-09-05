---
id: 20
title: Keep read commands working when one file is broken
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

One malformed file makes `list`, `next`, `board`, `search` and `render` all
fail. Confirmed: a truncated item takes down every read command.

That was a deliberate choice — silently skipping a broken file hides it — but
it is the wrong trade for read-only commands. An agent that cannot list the
backlog because somebody's editor crashed is blocked on an unrelated problem.

## Proposal

Read-only commands report the broken file on stderr and carry on with the rest.
`cairn check` still treats it as an error, and mutating commands still refuse,
because those are the paths where acting on a partial view does damage.

## Acceptance criteria

- [ ] `list`, `next`, `board`, `search` succeed with one unparseable file present
- [ ] The file is named on stderr every time, never swallowed
- [ ] `check` still fails; `set` and `renumber` still refuse

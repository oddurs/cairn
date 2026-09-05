---
id: 46
title: Change several items in one command
type: bug
status: backlog
milestone: v0.2
created: 2026-09-05
updated: 2026-09-05
priority: p1
effort: s
---

## Problem

`cairn close`, `cairn reopen`, `cairn release` and `cairn remove` all take a
list of ids. `cairn set` — the most-used mutating command — takes exactly one.

    cairn close 1 2 3          works
    cairn set 1 2 3 status=x   does not

So the common case, triaging a handful of items the same way, is a shell loop
the tool should have absorbed. The inconsistency is also the kind that makes a
tool feel unfinished: two commands next to each other in `--help`, one of which
accepts a list.

## Proposal

Accept several ids, and a filter for the case where you do not want to type
them:

    cairn set 1 2 3 priority=p0
    cairn set --filter 'milestone=v0.1,status=backlog' priority=p1

A filtered write is the dangerous one, so it prints what it matched and asks
before touching anything unless `--yes` is given — the same shape `remove`
already uses. Every write still happens under one lock, so a bulk edit is not a
window during which the backlog is half-changed.

## Acceptance criteria

- [ ] Several ids accepted, matching the commands that already do
- [ ] `--filter` selects, and confirms before writing unless `--yes`
- [ ] One lock for the whole operation
- [ ] A failure part-way names what was written and what was not

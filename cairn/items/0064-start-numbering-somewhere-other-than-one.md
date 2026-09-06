---
id: 64
title: Start numbering somewhere other than one
type: feature
status: backlog
milestone: v0.2
created: 2026-09-06
updated: 2026-09-06
priority: p2
sprint: s6
effort: s
---

## Problem

Allocation is `max(id) + 1`, so every project's first item is `1`. A team that
wants `MP-1002` cannot get there without creating a thousand items or editing
files by hand.

There is a smaller reason too: single-digit identifiers age badly. `MP-7` and
`MP-1002` in the same list read as different kinds of thing, and starting at a
round number avoids it.

## Proposal

```toml
[project]
id_start = 1000
```

The first item allocated is `id_start`; afterwards allocation is unchanged.
Lowering it in a project that already has items does nothing, because allocation
still takes the maximum — which is the right behaviour and should be said rather
than left to be discovered.

## Why this is separate

It is genuinely optional. Someone who wants `A24` does not need it, and the
identifier format is useful without it. Filed apart so it can be dropped without
taking anything else with it.

## Acceptance criteria

- [ ] The first item in an empty project takes `id_start`
- [ ] An existing project is unaffected by adding or changing it
- [ ] The manual says what happens if it is lowered

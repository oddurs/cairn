---
id: 73
title: The hierarchy is a query, not a schema
type: feature
status: backlog
milestone: v1.0
depends_on:
- 72
- 79
created: 2026-09-06
updated: 2026-09-07
priority: p1
effort: m
sprint: s8
---

## Problem

Once items compose (`0072`), the obvious next move is a `scale` field —
initiative, epic, story, task — so that a big thing can be labelled big.

It is a lie that goes stale. You tag something an epic, it turns out to be one
afternoon's work, and now the tag is wrong and nobody will fix it. Meanwhile
"has fourteen things part_of it" is *observably* an epic and cannot be wrong.

The same argument applies to `depth`, `progress` and everything else about an
item's position: they are facts about the graph, and the graph already knows
them.

## Proposal

No new stored fields. New derived pseudo-fields in the filter language, beside
the `category`, `blocked`, `ready` and `blockers` that already exist:

| field | is |
| --- | --- |
| `contains` | the items directly part of this one |
| `descendants` | how many items are beneath it at any depth |
| `progress` | proportion of descendants in a `done` category |
| `depth` | how far it sits below a root |
| `leaf` | whether anything is part of it |

Then the queries people actually want are one line and nobody classified
anything:

```sh
cairn list --filter 'depth=0,progress<50'      # big things that are behind
cairn list --filter 'leaf=true,ready=true'     # work that can start today
cairn next --filter 'descendants=0'            # nothing that is really a bucket
```

`cairn roadmap` and `cairn board` should show progress from this rather than
from a count of items carrying a milestone name.

## Why this is filed apart from 0072

It is pure query — no format change, nothing written to disk, nothing to
migrate. It can land, be wrong, and be changed again without costing anybody a
migration, which is not true of the relation itself.

## Acceptance criteria

- [ ] Every field above is available to `--filter` and `--sort`
- [ ] Nothing new is stored in an item file
- [ ] Cycles cannot make a derivation loop forever
- [ ] A project using no relations sees no change
- [ ] The manual explains why scale is derived rather than declared

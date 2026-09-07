---
id: 88
title: Prefer schema over documented keys
type: docs
status: backlog
milestone: v0.2
created: 2026-09-07
updated: 2026-09-07
priority: p1
effort: s
sprint: s9
---

## Problem

Format 2 was cheap and format 3 might not be, and the difference is worth
naming because it is a rule rather than an accident.

Format 2 moved milestones from `cairn.toml` into items. It touched **zero**
item files across seven real projects. The whole cost was one configuration file
per project and a command nobody had to think hard about.

The reason is simple: what moved was *schema*, and schema is per-project. What
did not move was the *format*, and the format is everybody's.

## The rule

> **Prefer expressing a thing as schema or as an item, rather than as a
> documented key.**
>
> A documented key is a promise to everyone who ever writes a cairn file.
> Changing one costs a format number and a migration for every project in the
> world. Schema is a promise to one project, and changing it costs that project
> a line of TOML.

It is the same instinct that made status *categories* fixed while status
*names* are chosen: the fixed axis is small and permanent, and everything
expressive sits above it.

## What this predicts

It says a milestone should have become an item --- which it did, and the cost
proved the rule. It says `part_of` was right to be a declared field rather than
a new documented key. And it says the next request that sounds like "cairn
should have a field for X" is usually answered by a `[[field]]` block, not by
§4.

## What would still cost a format bump

Written down so it is decided rather than discovered.

| Change | Cost |
| --- | --- |
| `id` stops being an unsigned integer (`0067`) | **every item file, every project** |
| Removing `milestone` or `depends_on` as documented keys | configuration, like format 2 |
| `created` / `updated` gaining a time | every item that has one |
| `labels` becoming references rather than strings | every item that has one |
| Any new **optional** key | free, no bump |
| `assignee` accepting a sequence as well as a string | free --- `labels` already sets that precedent |

Only the first is expensive, and it is the only one where the cost grows with
adoption. It is why `0067` is a decision to make now rather than a feature to
build later.

## Acceptance criteria

- [ ] The rule is in `CONTRIBUTING.md` beside the other three
- [ ] The table is in the manual, next to the compatibility rules
- [ ] A pull request adding a documented key has to say why schema would not do
- [ ] It reads as a design principle, not as a warning about paperwork

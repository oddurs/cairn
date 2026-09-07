---
id: 97
title: Adopt a schema instead of retyping it
type: feature
status: done
milestone: v0.2
created: 2026-09-07
updated: 2026-09-07
priority: p2
effort: m
area: init
---

## Problem

Across the seven projects that use cairn today, stripping comments and the
`[project]` block, **52 lines of schema are byte-identical in all seven** —
about half of the smallest configuration. The same four statuses, the same
priority and effort enums, the same milestone type and reference, the same
render block.

None of it was shared. It was retyped, or copied by hand, and it drifts: the
same idea is now spelled slightly differently in seven files, and a change to
the one that is right does not reach the other six.

`cairn init --preset standard` only offers what cairn ships with. The moment a
project has a schema worth having, there is no way to start the next project
from it.

## Proposal

```
cairn init --from ../other-project
```

Take that project's `cairn.toml`, keep its types, statuses, fields, views,
render block and hooks, and replace `[project]` with this project's own name and
directory. Milestones are items now, so nothing about somebody else's schedule
comes along.

That is a flag on a command that already exists, which is the bar a new surface
has to clear. It solves the real problem — adopting a schema — without inventing
indirection.

## What this is deliberately not

**Not `extends`.** An include mechanism would make `cairn.toml` no longer the
whole truth about a project: reading it would mean resolving a path, which may
be outside the repository, may not exist on a clone, and may have changed since.
The file being self-contained is what lets somebody who clones the repository
understand the backlog, and that is worth more than saving a copy.

Copying has a cost — the seven files still drift — and it is the right cost to
pay. A schema that two projects share is two projects that cannot change
independently.

**Not a registry.** `--from` takes a path. Fetching a schema over a network is
not something this tool should learn to do.

## Acceptance criteria

- [x] `cairn init --from PATH` starts from that project's schema
- [x] `[project]` is this project's own: name, directory, url, nothing inherited
- [x] Milestones do not come along, because they are items
- [x] Comments in the source file survive the copy, since they are half of what makes a schema legible
- [x] It refuses a path that is not a cairn project, and says so
- [x] The manual explains why this is a copy rather than an include

## 2026-09-07

Done. `cairn init --from PATH` copies the schema with toml_edit, so the comments survive; `[project]` is replaced with this project's own name and directory, and a description and url are dropped. `render.link_items` is turned off with the url so the new project does not arrive failing its own check, and starter milestones are only written when the adopted schema declares the type and field for them. A source at an older format is refused and told to migrate first, rather than having the migration quietly duplicated here.

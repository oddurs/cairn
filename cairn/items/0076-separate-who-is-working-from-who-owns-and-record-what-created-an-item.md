---
id: 76
title: Separate who is working from who owns, and record what created an item
type: feature
status: backlog
milestone: v1.0
created: 2026-09-06
updated: 2026-09-06
priority: p1
effort: m
sprint: s8
---

## Problem

`assignee` is one string doing two jobs: who is working on this, and who is
answerable for it. With a person that conflation is harmless, because the two
are usually the same person.

With agents it is the normal case that they differ. An agent is working; a human
owns. Today claiming an item overwrites the owner with the worker, so the moment
an agent picks something up, the record of who cares about it is gone.

There is a second gap beside it. Nothing records what *created* an item. As the
proportion written by agents rises, the query people will want most is "items no
human has looked at", and it is currently unanswerable.

## Proposal

Two things, both additive.

`assignee` keeps its meaning — who is working — and `owner` is added for who is
answerable. `cairn claim` sets `assignee` and leaves `owner` alone. A person
filing an item becomes its owner by default; an agent filing one leaves it
unowned, which is a useful thing to be able to find.

And provenance, recorded rather than inferred:

```yaml
created_by: claude        # what wrote this
```

`CAIRN_USER` already supplies the name, and the MCP server already knows the
caller from `clientInfo.name`. This writes down what cairn is told rather than
asking for anything new.

## What this is not

Not attribution for its own sake, and not a permissions model — `0075` covers
what an agent may do. This is about being able to ask two questions that cannot
be asked today: who cares about this, and did a person write it.

## Acceptance criteria

- [ ] `owner` exists beside `assignee`, and `claim` does not overwrite it
- [ ] `created_by` records what created an item, from the identity cairn is given
- [ ] Both are filterable: `owner=`, `created_by=claude`
- [ ] `cairn next --mine` means owned or assigned, and says which
- [ ] Existing items with only `assignee` behave exactly as they do now
- [ ] No format bump: both keys are optional additions

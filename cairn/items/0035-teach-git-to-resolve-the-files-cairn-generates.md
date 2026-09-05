---
id: 35
title: Teach git to resolve the files cairn generates
type: feature
status: done
milestone: v0.2
assignee: claude
created: 2026-09-05
updated: 2026-09-05
priority: p0
effort: m
---

## Problem

Two branches, each adding one item, merged:

    CONFLICT (content): Merge conflict in ROADMAP.md
    cairn: id 0002 is used by 2 files — run `cairn renumber`

Both, on the first parallel merge. This is the largest friction in the workflow
cairn is built for, because agents work on branches and branches are exactly
where the repository lock does not reach.

Neither is really a conflict:

- `ROADMAP.md` is generated. Merging its text is meaningless — there is a
  correct answer and it is "render it again".
- Ids collide because allocation reads the highest in use, which no branch can
  know. Git merges the files cleanly, since the filenames differ, and leaves a
  project that fails `check`.

## Proposal

`cairn init --git`, installing two things:

    # .gitattributes
    ROADMAP.md merge=cairn

A merge driver that discards both sides and re-renders, and a `post-merge` hook
that runs `cairn renumber` when identifiers collided and re-renders afterwards.

Neither invents a resolution. Both re-derive a file from the items, which are
the only thing that was ever authoritative.

## Why this and not a different identifier scheme

Content-addressed ids would remove the collision and the ergonomics with it:
`cairn set k3f9a2 status=done` is not a command anyone wants to type. Small
integers are worth keeping, and a merge is the right moment to reconcile them.

## Acceptance criteria

- [ ] Two branches adding items merge with no conflict and no manual step
- [ ] `cairn init --git` is idempotent and prints what it changed
- [ ] An existing project can adopt it without re-running `init`
- [ ] Works when the merge is done by a forge rather than locally, or says
      plainly that it cannot
- [ ] Tested by actually merging branches, not by inspecting configuration

## 2026-09-05

Built. Merge driver keeps our side of the generated roadmap; post-merge hook renumbers collisions and re-renders once the items are settled. The ordering constraint — git resolves ROADMAP.md before the items it is rendered from — is why the driver cannot simply render.

---
id: 152
uid: 4ce47630-1c4f-4db1-bc0a-4f012a5842a1
title: Match other worktrees' copies by uid in format 5
type: bug
status: done
milestone: v1.0
assignee: Oddur Sigurdsson
created: 2026-09-27
updated: 2026-09-27
closed_at: 2026-09-27
priority: p1
area: git
effort: s
---

## Problem

The worktree survey (#107) pairs another worktree's copy of an item with ours
by `id`. In format 4 that was the UUID and could not differ. In format 5 it is
a number, and a number can change or collide:

- A merge repaired by `cairn renumber` leaves the same item under different
  numbers in two worktrees. Its claim there was invisible here, so
  `claim --next` could take it again.
- Two items given one number on different branches read as one item. A claim
  on the other would refuse this one.

Harrow already matches by `uid` first (harrow#103), so the two disagreed.

## Proposal

Decide once, when the survey is taken, which item here each copy is a copy
of: the item with the same `uid` where both carry one; otherwise the item with
the same number, unless that item carries a different `uid`. A copy with no
counterpart is reported as filed there.

## Acceptance criteria

- [x] A copy renumbered in another worktree still holds its item here
- [x] The same number on a differently tagged item is not a copy, and holds nothing
- [x] `make check` passes

## 2026-09-27

Copies pair with items by uid first, then by id unless the item here carries a different uid. Two real-git tests (renumbered copy still holds; colliding number is filed-there) fail with id-only matching and pass with this. make check: 630 passed; clippy -D warnings clean.

---
id: 102
title: Unit-test the reference machinery
type: chore
status: backlog
milestone: v0.2
created: 2026-09-07
updated: 2026-09-07
priority: p1
sprint: s10
effort: m
area: refs
---

## Problem

`refs.rs` is 612 lines with **no unit tests**. It is the newest machinery in the
project and the thing `check`, `roadmap`, `board`, `next`, `show` and `remove`
all lean on.

`would_cycle`, `depth`, `permitted`, `targets`, `resolve` and `rename_key` are
pure functions over a graph. They have interesting edge cases — a self
reference, a cycle closed at depth five, a key that differs only in case, a ref
whose target type was deleted from the schema — and every one of those cases is
currently reachable only by building a project on disk and running a command
against it.

That is slow, indirect, and it means a failure arrives as "`cairn check` said
something odd" rather than as "`would_cycle` is wrong at depth 5".

## Proposal

Unit tests, in the file, over hand-built `Item` values. No temporary directories,
no process spawning, no schema loaded from disk beyond what the functions need.

The cases worth naming:

- `would_cycle`: a self reference; a two-item cycle; a cycle closed at depth 5;
  a diamond, which is not a cycle and must not be reported as one
- `depth`: a leaf; a chain; an item whose parent is missing
- `resolve`: by id and by key; a key differing only in case; a key that is also a
  valid identifier under the project's `id_format`
- `permitted`: a target type that no longer exists in the schema
- `rename_key`: every referrer updated, and one that referred by id left alone
- `Milestones::new`: no milestone field declared; a field declared with a
  different target

## Why this is cheap and worth doing now

`refs.rs` arrived recently and will be extended — it is the general mechanism
that container types, rollups and inverses are all built on. Direct tests are
much cheaper to write against it today, while the shape is fresh, than after the
next thing is layered on top.

## Acceptance criteria

- [ ] Every pure function in `refs.rs` has direct unit tests
- [ ] Cycle detection is tested at depth 1, 2 and 5, and a diamond is not a cycle
- [ ] Key resolution is tested for case, and for a key shaped like an identifier
- [ ] `rename_key` is tested for a referrer by key and one by id
- [ ] None of them build a project on disk

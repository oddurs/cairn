---
id: c5ed8586-b105-44b1-b630-874997def019
title: Readable ids with a hidden identity tag
type: decision
status: done
milestone: v1.0
created: 2026-09-26
updated: 2026-09-26
closed_at: 2026-09-26
priority: p1
area: format
---

## Decision

Format 5 makes `id` a readable, configurable number again and keeps the format-4
UUID as a hidden tag, `uid`. This supersedes the identity half of 0067.

The owner's reason: UUIDs are fine as a tag, but they are poor for visibility —
reading a table, a file, a commit trailer, or a `depends_on` list. What people
see, type and grep should be the configured id.

## What format 5 is

- `id` is an unsigned integer, rendered by the project's `id_format` template
  (`0042`, `RIM-42`). A type may declare its own template (`BUG-{n}`). Every
  type shares one counter, so the number alone is always unique and a retyped
  item keeps its identity; only its rendering changes.
- `uid` is a UUIDv4 written once at creation and never displayed by default.
  It is what survives a renumber: history continuity, interchange, and exact
  repair after a merge collision. Old UUID references keep resolving through it.
- References (`depends_on`, id-addressed fields) store the number, so a file
  reads the way the table does.

## Collisions

Format 4 existed to stop parallel branches taking the same number. Format 5
answers it in two layers:

1. **Avoid.** Allocation takes one more than the highest number anywhere the
   repository knows: the working tree, every linked worktree, and every item
   file ever added on any local or remote-tracking branch. Parallel agents on one
   machine, one worktree each, cannot collide, and a deleted item's number is
   never reused.
2. **Repair.** A collision between machines is renumbered at merge, keeping the
   published side's number. The uid identifies the moved item on both sides, so
   references that arrived with it are retargeted instead of left pointing at the
   wrong item.

## Rejected

- Per-type counters (`BUG-1`, `FEAT-1`): the stored id would need the prefix,
  and retyping would change identity.
- Per-writer prefixes (`a12`): collision-free, but loses creation order and
  needs a registry of writers.
- Keeping UUIDs visible with shorter handles: the owner's objection is to
  visibility, and a handle that grows as the project grows does not answer it.

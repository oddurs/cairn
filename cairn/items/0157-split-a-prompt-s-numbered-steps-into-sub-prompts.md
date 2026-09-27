---
id: 157
uid: 7a1337cf-c041-403f-bf5b-ca4a28131280
title: Split a prompt's numbered steps into sub-prompts
type: feature
status: planned
milestone: prompts
depends_on:
- 154
created: 2026-09-27
updated: 2026-09-27
priority: p2
area: cli
effort: m
---

## Problem

0147's Approach was three numbered steps; each became separate work in another
repository, but none became an item. They could not be claimed, ticked or
blocked on one at a time, and the order between them lived in prose.

## Approach

`cairn split <ID>` reads the numbered list in the item's Approach (`--from
SECTION` for another) and creates one child item per top-level step:

- title: the step's first line, without Markdown emphasis, shortened to fit;
- body: the whole step, and the parent's id as where it came from;
- type, milestone and priority from the parent;
- each child depends on the one before it (`--parallel` for none);
- the parent depends on every child, so it cannot finish first; and where the
  schema declares a rollup id reference (`part_of` in the standard preset),
  each child also names the parent through it;
- a note on the parent lists what it was split into.

`--dry-run` prints the children without writing. Refuses an item with no
numbered steps, or one already split, saying why.

## Acceptance criteria

- [ ] Splitting creates one child per top-level numbered step, in order, each depending on the one before
- [ ] `--parallel` creates them with no dependencies between them
- [ ] The parent depends on every child, and names them in a note
- [ ] With a rollup id field in the schema, each child names the parent through it
- [ ] `--dry-run` writes nothing; an item with no steps, or already split, is refused with the reason
- [ ] `make check` passes

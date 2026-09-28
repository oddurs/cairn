---
id: 157
uid: 7a1337cf-c041-403f-bf5b-ca4a28131280
title: Split a prompt's numbered steps into sub-prompts
type: feature
status: done
milestone: prompts
assignee: oddurs
depends_on:
- 154
created: 2026-09-27
updated: 2026-09-27
closed_at: 2026-09-27
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

- [x] Splitting creates one child per top-level numbered step, in order, each depending on the one before
- [x] `--parallel` creates them with no dependencies between them
- [x] The parent depends on every child, and names them in a note
- [x] With a rollup id field in the schema, each child names the parent through it
- [x] `--dry-run` writes nothing; an item with no steps, or already split, is refused with the reason
- [x] `make check` passes

## 2026-09-27

Dry run against a copy of this backlog: split 0147 --dry-run proposes 0159 'In the harrow repository, file and do the reader change', 0160 'Install the matching pair' after 0159, 0161 'Migrate harrow, rim and nun, each in its own commit, verifying restored numbers against…' after 0160. That run caught titles being cut at a wrapped line; a title is now the first sentence across the step's lines.

## 2026-09-27

Review (/code-review medium) found five real defects, each fixed with a test (four integration, one unit) that fails without the fix: children skipped schema defaults and required fields (now filed as new files them, inheriting priority and required values from the parent, refusing before any write if one is missing); a container's steps became keyless milestones (they take the default type); any paragraph beginning 'Split into' blocked splitting (only a note counts); a mid-way refusal could leave orphaned children (everything is validated against the future backlog before the first write); fenced examples and a later list became steps (fences skipped, the list ends at the first paragraph back at the margin).

## Result

cairn split <ID> makes each top-level numbered step of an item's Approach (or --from SECTION) an item: titled by its first sentence, holding the whole step, filed as new would file it plus the parent's priority and required fields, each depending on the one before (--parallel for none). The parent depends on all of them and records the split in a note, which refuses a second split; a rollup id field such as part_of names the parent. --dry-run writes nothing.

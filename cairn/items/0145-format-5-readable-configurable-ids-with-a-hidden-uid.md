---
id: 145
uid: 49543fc8-23b5-4987-acd9-4a1c680693e3
title: 'Format 5: readable configurable ids with a hidden uid'
type: feature
status: done
milestone: v1.0
assignee: oddurs
depends_on:
- 146
created: 2026-09-26
updated: 2026-09-26
closed_at: 2026-09-26
priority: p1
area: format
effort: xl
---

## Problem

Format 4 shows UUID prefixes everywhere an id appears: tables, filenames,
`depends_on`, commit trailers. The decision in the linked item returns to
readable, configurable numbers with the UUID kept as a hidden `uid` tag.

## Approach

- Format 5: numeric `id` with project and per-type `id_format` templates; `uid`
  written on create, carried through every write, shown only in JSON.
- Commands accept the bare number, any configured rendering, a full uid, and a
  uid prefix of at least eight hex digits that is not all digits.
- Allocation looks across worktrees and every branch's item history.
- `renumber` at a merge retargets references that arrived with the moved item,
  matching items across sides by uid.
- Migration 4→5 restores every legacy number from `_legacy-ids.toml`, numbers
  items created since in creation order, rewrites references, renames UUID
  filenames, restores `id_format`, and removes the frozen map. Migration 3→5
  tags every item with a uid. Both are journaled and resumable.
- Format 4 becomes read-only in this build: reading works, writing asks for
  `cairn migrate`, as every older format already does.

## Acceptance criteria

- [x] Specification describes format 5; the format-4 contract is frozen beside format 3
- [x] New items get the next number across worktrees and branch history, plus a uid
- [x] A type's `id_format` renders and parses; a prefix naming the wrong type is refused
- [x] Commands resolve numbers, renderings, full uids and uid prefixes
- [x] A merge collision renumbers the arriving item and retargets references that arrived with it
- [x] Migration 4→5 restores legacy numbers, numbers the rest by creation, renames files, and is resumable
- [x] Migration 3→5 tags every item and changes nothing else in them
- [x] cairn's own backlog is migrated in its own commit, with numbers and history verified
- [x] `make check` and `make durability` pass

## 2026-09-26

Per-type rendering is a type-level id_format ("BUG-{n}"), not the id_prefix key the design sketch showed: the project already argues for one template over separate prefix/separator/padding settings, and a template can pad. A type template must carry its own prefix or suffix, distinct from every other, because the prefix is checked: BUG-13 for a feature is refused rather than reinterpreted.

## 2026-09-26

Two real bugs found by the restored format-3 tests. (1) arrival_side asked git log --diff-filter=A --follow which commit added each file; rename detection read a new item as a rename of an old one (same boilerplate), so both sides of a collision looked published and the local item was renumbered. Latent since format 3; format 4 hid it. Now decided by uid against the two parents' trees. (2) Loading the store replaced the identity index, so validate_on_write's reload forgot the item new had just stamped and it printed 0002 instead of BUG-2; loads now merge.

## 2026-09-26

renumber does not tag hand-written untagged items: doing so at every post-merge left unrelated uncommitted changes after ordinary merges (found by at_a_merge_the_side_already_published_keeps_its_identifier). Only cairn-created items carry a uid. CAIRN_ITEM_ID is the number (format 4 meaning, machine id) and CAIRN_ITEM_REF the rendering; format 3 put the rendering in CAIRN_ITEM_ID. Verified: make check green; make durability pieces green — 400-op and concurrent soak, 20000 fuzz vectors, conformance 63 cases over current + formats 1-4; recordings regenerate byte-identically.

## 2026-09-26

Migrated this backlog in d24f7f5. Independent check against the removed _legacy-ids.toml: 144 aliases restored exactly, 147 bodies byte-identical, every other frontmatter value unchanged, depends_on mapped through the same table; new items are 0145-0147. log --range over the migration reports nothing changed; item 67's history runs unbroken from format 3 through 4 into 5, and old UUID prefixes (49543fc8) still resolve. make check green after migration.

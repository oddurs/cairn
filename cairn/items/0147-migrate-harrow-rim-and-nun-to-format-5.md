---
id: 147
uid: c7ac551e-b7f3-4da0-b7a1-94fd027ea098
title: Migrate harrow, rim and nun to format 5
type: chore
status: backlog
milestone: v1.0
depends_on:
- 145
created: 2026-09-26
updated: 2026-09-26
priority: p0
area: format
---

## Problem

harrow, rim and nun are format 4. This build makes format 4 read-only, so they
need `cairn migrate` to format 5 — but Harrow must read format 5 first, or its
board goes dark in all three.

## Approach

1. In the harrow repository, file and do the reader change: numeric `id`, `uid`
   carried through, type `id_format` templates. Pin agreement with cairn.
2. Install the matching pair.
3. Migrate harrow, rim and nun, each in its own commit, verifying restored
   numbers against `_legacy-ids.toml` before it is removed.

## Acceptance criteria

- [ ] Harrow reads and writes format 5, tracked in its own repository
- [ ] harrow, rim and nun are format 5, each migrated in its own commit

## 2026-09-26

Order changed: the pinned Harrow refuses format 5, and CI's agreement gate runs on every Cairn PR, so the format-5 PR cannot merge green until Harrow reads format 5 and spec/harrow-revision moves. Measured with make agreement against pin 0a5b2df: 3 of 5 pass; the_cairn_projects_saved_views_agree_through_machine_output and migrated_aliases_and_native_uuid_queries_agree fail with 'unsupported Cairn format 5'. The second is a format-4 fixture and needs replacing with a format-5 one on both sides. Do step 1 before merging the format-5 PR; steps 2-3 after.

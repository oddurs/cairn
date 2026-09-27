---
id: c7ac551e-b7f3-4da0-b7a1-94fd027ea098
title: Migrate harrow, rim and nun to format 5
type: chore
status: backlog
milestone: v1.0
depends_on:
- 49543fc8-23b5-4987-acd9-4a1c680693e3
created: 2026-09-26
updated: 2026-09-26
priority: p1
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

---
id: b63904a8-f775-4ef4-8a0c-123971a2ed89
title: Advance the Harrow pin to worktree status and the pin it reciprocates
type: chore
status: done
milestone: v1.0
assignee: Oddur Sigurdsson
created: 2026-09-27
updated: 2026-09-27
closed_at: 2026-09-27
priority: p1
area: integration
effort: s
---

## Problem

`spec/harrow-revision` pinned Harrow `0a5b2df`, from before Harrow read other
worktrees (harrow#101) and before it pinned today's Cairn (harrow#102). The
agreement gate was comparing Cairn `main` with a Harrow that no longer exists
on Harrow's `main`.

## Proposal

Pin Harrow `426d43f`, its `main` after #102. Not the newer `main` with the
format-5 reader (harrow#103): that Harrow's migration test expects `cairn
migrate` to reach format 5, which this Cairn does not write. The format-5 PR
(#105) moves the pin to a Harrow at or after harrow#103.

## Acceptance criteria

- [x] `make agreement` passes against Harrow `426d43f`
- [x] `make check` passes

## 2026-09-27

make agreement against Harrow 426d43f: 5/5. make check: 619 passed, clippy -D warnings clean, check --render --strict ok.

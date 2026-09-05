---
id: 25
title: Make claim atomic against a concurrent claimer
type: bug
status: done
milestone: v0.2
assignee: claude
created: 2026-09-04
updated: 2026-09-04
priority: p1
sprint: s3
effort: s
---

## Problem

`cairn claim` reads the item, checks the assignee, then writes. Two agents can
both pass the check before either writes, and the second silently wins.

## Proposal

Perform the check and the write under the repository lock, and re-read the item
inside it so the decision is made on current state.

## Acceptance criteria

- [ ] Two simultaneous claims: exactly one succeeds, the other is told why
- [ ] Covered by the concurrency stress test

---
id: 28
title: Keep a golden corpus of items that must always parse
type: chore
status: done
milestone: v1.0
assignee: claude
created: 2026-09-04
updated: 2026-09-04
priority: p0
sprint: s4
effort: s
---

## Problem

Nothing stops a refactor from quietly changing how a file is interpreted.

## Proposal

A committed corpus of item files — ordinary ones, awkward ones, every historical
shape — with expected parse results. CI asserts they parse identically forever.
Every format bug found adds a case.

## Acceptance criteria

- [ ] Corpus covers every documented key and every known edge case
- [ ] Byte-for-byte render comparison, not just field equality
- [ ] A deliberate parser change fails the corpus test

---
id: 49
title: A cookbook of things people actually want to do
type: docs
status: backlog
milestone: v0.2
created: 2026-09-05
updated: 2026-09-05
priority: p2
effort: m
---

## Problem

The manual is reference and the site is a tour. Neither answers "how do I do
the thing I came here to do". A tool becomes the obvious choice partly by
having an answer ready for the question somebody is about to ask.

## Proposal

Short recipes, each a real command sequence, on the site and in the manual:

- Adopt cairn in a project that already has GitHub issues
- Run a weekly triage without typing an identifier twice
- Wire it into CI so the roadmap cannot drift
- Give an agent a scoped slice of the backlog to work on
- Move a project between two different schemas
- Find everything one person is holding
- Recover after a bad merge

Each recipe should be shorter than the explanation of why it works, and should
be built from recorded output rather than typed.

## Acceptance criteria

- [ ] Every recipe is a sequence somebody could paste
- [ ] Output is recorded, not written
- [ ] Reachable from the site and from `info cairn`

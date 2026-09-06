---
id: 70
title: Change the specification before changing the format
type: chore
status: backlog
milestone: v0.2
created: 2026-09-06
updated: 2026-09-06
priority: p2
sprint: s7
effort: s
---

## Problem

The specification has so far been written after the code, describing what cairn
already did. That worked because one person wrote both within a few days of each
other, and it will stop working the moment that is not true.

A specification that trails an implementation is a description. A specification
that leads it is a design tool — and it caught something already: writing down
that `id` is an unsigned integer is what made the identifier-prefix design
obvious three days later, by ruling out the answer that looked right.

## Proposal

A rule, in `CONTRIBUTING.md`: a change touching what is written to disk edits
`spec/README.md` first, in the same pull request, before the code.

Two things follow. Somebody must decide whether the change is additive — and
therefore free under the compatibility rules — before building it rather than
after. And the pull request shows the intended contract next to the code
implementing it, which is the right place for a reviewer to disagree.

## Acceptance criteria

- [ ] The rule is in CONTRIBUTING, next to the compatibility promise
- [ ] The pull request template asks whether the format changed
- [ ] A change to what is written to disk without a specification edit is
      questioned in review

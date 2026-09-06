---
id: 57
title: Renumber should keep the identifier the published side already has
type: bug
status: backlog
milestone: v0.2
created: 2026-09-06
updated: 2026-09-06
priority: p1
effort: s
---

## Problem

Found while rebasing two branches that had each allocated `0055` — the exact
case `renumber` exists for.

It renumbered the wrong one. The rule is "the oldest item keeps the contested
identifier, and the path breaks a tie", which is right when two branches meet
as equals. At a rebase or a merge they are not equals: one side is already on
the main branch and has been published, and the other is arriving. Renaming the
published one churns history and, worse, breaks any link anybody has already
written to it.

Both items looked identical to cairn — same creation date, distinguished only
by filename — so it picked alphabetically and got it backwards.

## Proposal

When the project is a git repository, prefer the item that already exists in
the merge base. That is knowable: an item whose file is present in
`git merge-base HEAD MERGE_HEAD` was published first, whatever its creation date
says.

Outside a repository, or when neither side is in the base, fall back to the
current rule.

This shares its machinery with `0045`, which teaches cairn to read history at
all, so it is probably cheaper after that lands than before.

## Acceptance criteria

- [ ] At a merge, the side already in the base keeps its identifier
- [ ] Outside a repository the behaviour is unchanged
- [ ] A test performs a real merge of two branches that both allocated the same
      identifier, and asserts which one moved
- [ ] The manual says which side wins and why

---
id: 67
title: Decide whether identifiers should be collision-free by construction
type: feature
status: backlog
milestone: v1.0
created: 2026-09-06
updated: 2026-09-06
priority: p1
sprint: s7
effort: m
---

## Problem

Identifiers are allocated as one more than the highest in use, which needs
coordination that branches cannot provide. Managing that has cost a repository
lock, `renumber`, a merge driver, a post-merge hook and `0057` — a lot of
machinery to repair a problem rather than prevent it.

An identifier that carried a per-clone prefix — `a12`, `b7` — could not collide
at all. Short, readable, sayable, and the machinery above becomes unnecessary.

## Why this is not simply better

It conflicts with a promise made in this repository three days ago. The
specification commits to `id` being an unsigned integer, and the compatibility
rules say changing what a key means requires a new format version and a
migration.

So the appealing design costs a major version. That is not a reason to reject
it — 1.0 has not happened, and this is exactly the moment such a change is
cheapest — but it is a reason to decide deliberately rather than to drift into
it.

## What has to be weighed

- **Sequential identifiers carry information.** `0001` was created before
  `0002`. A prefixed scheme loses that, and nobody has established how much it
  is worth.
- **Alternatives inside format 1** exist and are worse: allocating from a random
  range makes collisions unlikely rather than impossible and still loses
  ordering; striping the number space per clone needs to know how many clones
  there are.
- **The machinery already works.** Collisions are detected, repaired, and
  repaired automatically at a merge. The cost of keeping the current design is
  known; the cost of changing it is a format version.
- **`0062` is unaffected either way.** How an identifier is rendered is separate
  from how it is allocated.

## Proposal

Decide before 1.0, because after 1.0 the price goes up and stays up. Write the
reasoning down whichever way it goes, so nobody relitigates it in a year.

Recommendation: probably keep integers, because the ordering property is real
and the repair machinery already exists and is tested. But that is a
recommendation from the person who built the repair machinery, which is exactly
the bias to be suspicious of, so it should not be decided alone.

## Acceptance criteria

- [ ] Decided before 1.0, with the reasoning recorded
- [ ] If changed: a format version, a migration, and the corpus extended
- [ ] If kept: the specification says why, so the question is settled
- [ ] Somebody other than the author reads the argument first

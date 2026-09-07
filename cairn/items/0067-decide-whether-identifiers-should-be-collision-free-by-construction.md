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

## 2026-09-06: three pieces of evidence this decision did not have

Recorded because they all arrived after the argument above was written, and two
of them point the same way.

**Identifiers being integers, and present in the file, is load-bearing in a
place nobody predicted.** `cairn log` follows renames, so that retitling an item
does not lose its history. Git's rename detection concludes that item 2 was
renamed from item 1 — every cairn item has the same shape, and a fresh one is
largely boilerplate — and follows into a different item's history. The
correction is to read the `id` at each revision and cut the trail where it
changes. Any scheme where identity is not a small stable value carried in the
file would have to answer this, and per-clone prefixes would have to answer it
without the ordering property that makes `max + 1` allocation work.

**The rendering answer already covers most of the demand.** `0062` shipped:
a project can write its identifiers `MP-1002` or `A24` while `id` stays an
unsigned integer. That was the case people actually asked about, and it cost no
format change at all. The remaining case for per-clone identifiers is narrower
than it was: it is now purely about collisions, not about how identifiers read.

**The collision repair got better rather than staying broken.** `0057` taught
`renumber` to keep the identifier on the side already published, by asking the
repository which side a merge came through. The cost of collisions is therefore
lower than when this item was filed. That is an argument for the status quo, and
it is also exactly the argument a person who just built the repair machinery
would find persuasive — which is the bias the item already warns about, now with
a concrete instance of me being subject to it.

**Recommendation unchanged, confidence higher:** keep integers. But this still
needs somebody who did not write the repair machinery to read the argument,
and that has not happened.

## 2026-09-07: this is now the only expensive change left

`0088` inventories what would still cost a format number. Everything on that
list is configuration-shaped, like format 2 — which touched zero item files
across seven real projects — except this one.

Changing what `id` is rewrites **every item file in every project**, plus every
`depends_on` in all of them. It is the only known change whose cost grows with
adoption, and cairn currently has the smallest number of users it will ever
have.

That reframes the decision. It is not "should identifiers be collision-free",
weighed on its merits whenever somebody gets to it. It is: **this door closes a
little more every week, and the cheapest moment to walk through it is now.**

If the answer is what I still think it is — keep integers — then the right
outcome is not to leave the question open. It is to write the promise into §4:
`id` is an unsigned integer and will remain one. An open question invites a
future maintainer to reopen it at the worst possible time; a promise does not.

Closing a door deliberately is worth more than the flexibility of leaving it
ajar, and this item should end by doing one or the other.


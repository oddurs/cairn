---
id: 58
title: Write down what cairn will always do, before anyone is watching
type: docs
status: done
milestone: v0.2
created: 2026-09-06
updated: 2026-09-06
priority: p0
effort: s
---

## Problem

If a hosted product is ever built beside cairn, the question every prospective
user will ask — quietly, and then loudly — is whether the free thing is a funnel.

Elastic, Redis, HashiCorp and Terraform all answered that question the same way:
by moving the line once they had something to lose. The licence changed, the
trust did not come back, and in three of those four cases the community forked.
What they had in common was declaring the boundary *after* it was worth money.

cairn has no users. Declaring the boundary now costs nothing and is worth more
than declaring it at any later moment, because a promise made when it is
inconvenient to break is worth more than one made when it is convenient to keep.

## Proposal

A short, plain statement in the README, the manual and on the site. Something
close to:

> cairn is free software under the GNU General Public Licence, and will remain
> so. Everything a single repository can do — items, the schema, the board, the
> roadmap, agents, merging, import and export — is part of cairn, and is free.
> No capability that belongs in cairn will be held back for something else. If
> anything built beside cairn disappeared tomorrow, no cairn user would be
> affected.

That last sentence is the one carrying the weight. It is the difference between
a core that a business sits beside and a core that a business sits on top of.

## What this is not

It is not a promise never to charge for anything. It is a promise about where
the line is, and that the line does not move. Being specific about that is what
makes it credible; a vague reassurance is worth nothing and everybody knows it.

## Acceptance criteria

- [ ] Stated in README, manual and site, in the same words
- [ ] Says what would be true if anything built beside cairn vanished
- [ ] Written before there is anything on the other side of the line
- [ ] Short enough that somebody reads all of it

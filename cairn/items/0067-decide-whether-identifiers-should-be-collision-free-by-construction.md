---
id: 67
title: Decide whether identifiers should be collision-free by construction
type: decision
status: doing
milestone: v1.0
assignee: codex
claimed: 2026-09-23
depends_on:
- 138
created: 2026-09-06
updated: 2026-09-23
priority: p1
area: git
effort: xl
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

- [x] Owner-approved decision and tradeoffs recorded before 1.0
- [x] Format 4 specifies immutable UUIDv4, full machine references, unambiguous prefixes and frozen legacy aliases
- [x] Resumable migration preserves body bytes, dates, unknown metadata and historical identity without rewriting Git
- [x] Frozen formats 1–3 and the current corpus pass both independent readers
- [ ] Harrow preserves full identity across reads, queries, selection, writes, undo and history; paired CI pins agree
- [ ] Durability, contract, documentation and packaging checks pass
- [ ] Both live backlogs migrate in separate commits with their old aliases and Git history verified
- [ ] Verified matching binaries are installed; migration and development-version status are documented

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

## 2026-09-23

Assessment 0134: keep the current integer identifiers during the next cycle. No format change or permanent promise is justified by this review alone. Decide after 0138 has measured the actual branch/worktree failure modes, with independent human review as the existing criteria require. A collision-free identifier would not by itself coordinate claims.

## 2026-09-23

Evidence from 0138: real branch merges can repair numeric collisions deterministically while retaining every item, but references to colliding IDs still need branch-context review; rebase and cherry-pick need explicit repair. Shared-directory locks do not coordinate worktrees. This is a real ergonomic cost to weigh before 1.0, not grounds to change identity mid-v0.3. The executable cases are tests/collaboration.rs and the supported workflow is doc/COLLABORATION.md.

## 2026-09-23

Owner approved the decision and implementation: immutable UUIDv4 identities, canonical full references, unambiguous short command handles, no per-clone allocator or permanent second numbering system. Implement a format-4 migration with a preserved legacy-number map and historical lookup, update Harrow independently, and migrate this project in a dedicated commit without rewriting Git history. This explicit owner approval supersedes the earlier requirement to wait for outside-user review; the old rationale remains as history. The change belongs on a breaking-version development line, not in a 0.3 patch.

## 2026-09-23

Format 4 implementation is underway on feat/immutable-identities. UUIDv4 identities, full stored references, ambiguity-safe prefixes, a frozen legacy-number map, and resumable migration are implemented. The migration retains old filenames and exact body bytes; history comparisons bridge the migration without rewriting Git. Verified 65 core unit tests, 19 focused identity/harness tests, current golden parsing, and 49 independent-reader cases across formats 1-4. General regression tests are being updated to use actual UUIDs or explicit UUID fixtures, not rewritten command output. Harrow counterpart work is tracked in its item 0107 and separate worktree. Neither live project has been migrated yet.

## 2026-09-23

The format suite now passes all 43 tests, including migration of the frozen format-1 corpus. That test exposed and fixed comma-separated legacy dependency migration. The updated soak passed 400 mixed operations with identity-preserving export/import and unchanged identities through renumber; the concurrent soak created 100 distinct items. Hooks now expose full CAIRN_ITEM_ID plus short CAIRN_ITEM_REF. Harrow counterpart implementation is underway in its isolated feat/immutable-identities worktree; its core compiles with identity-bearing state separated from numeric counts. Live backlogs remain format 3 pending paired verification.

## 2026-09-23

The active criteria now express the owner-approved change rather than the superseded keep-integers alternative. Full Cairn default tests, release-mode 400-operation soak and 100-item concurrent creation pass; the 20,000-argument fuzz run is underway. Harrow standalone checks and all four explicit cross-tool agreement tests pass. New Harrow main work will be integrated before migration; the implementation remains separate from live-data conversion.

## 2026-09-23

Decision rationale: with one daily user and no external migration population, this is the cheapest point to remove coordinated numbering from the durable identity contract. UUIDv4 uses 122 random bits from OS entropy, not a clone registry or a clock; collision risk is negligible, not mathematically impossible, and duplicates still fail validation. Creation dates carry chronology, full UUIDs carry identity, and short prefixes are disposable command handles. The benefit is independent creation without renumbering or ambiguous dependency retargeting after merges. This does not solve simultaneous edits or coordinate claims across worktrees: Git review and explicit assignment remain necessary. Numeric aliases are a frozen migration bridge, never an ongoing second allocator.

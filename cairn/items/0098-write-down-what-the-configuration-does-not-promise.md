---
id: 98
title: Write down what the configuration does not promise
type: docs
status: backlog
milestone: v1.0
depends_on:
- 96
created: 2026-09-07
updated: 2026-09-07
priority: p2
effort: s
area: docs
---

## Problem

Four things about `cairn.toml` are true, load-bearing, and written down nowhere.
None of them is a defect. Each of them is something somebody will discover the
expensive way, and the cost of writing them down is a page.

**Order is meaning.** The order of `[[status]]` blocks is the column order on the
board and the sort order in every listing. The order of an enum's `values` is
what `--sort priority` sorts by. Nothing in the file says so, and a tool that
rewrote it alphabetically — or a person tidying it — would change behaviour
without changing anything they could see.

**`cairn.toml` is deliberately outside the merge driver.** `cairn git` registers
the driver for `cairn/items/*.md` and for the rendered roadmap, and not for the
configuration. That is the right decision: unioning two branches' schema blocks
would silently produce a schema neither branch wrote, and a schema conflict
wants a person. But it is the one file where a bad merge is quiet rather than
loud, and nobody is told.

**Some keys are semantic and some are decoration.** `category`, `values`,
`required`, `acyclic`, `rollup`, `target` and `by` change what a reader must
conclude. `color`, `icon`, `label`, `board` and `column` change what a reader
draws. §4.3 already leans on that distinction — a reader that ignores the
project's rendering conforms — but the configuration presents both in the same
block with the same weight, so a second implementer has to guess which half is
optional.

**Multiplicity is spelled twice.** `kind = "list"` for an ordinary field,
`cardinality = "many"` for a reference. The same idea, two names, and no way to
say a many-valued enum. `cardinality` on every kind would subsume `list`, but
`list` is a documented key and removing it costs a format number, so this is a
wart to record rather than a change to schedule.

## Proposal

A section of the manual — "What the configuration does not promise" — saying
these four things plainly, and the specification gaining the one that a second
implementer needs: which configuration keys are semantic.

Order is worth a comment in the shipped `cairn.toml` as well, next to the
statuses, because that is where somebody is standing when it matters.

## Why this is worth a ticket

Everything here is a thing I knew for an hour and would have forgotten. Written
down, the next person disagreeing with one of these decisions argues with the
reasoning instead of rediscovering the behaviour.

## Acceptance criteria

- [ ] The manual says the order of statuses and enum values is behaviour
- [ ] It says the configuration is deliberately outside the merge driver, and why
- [ ] The specification says which configuration keys a conforming reader must honour
- [ ] `kind = "list"` versus `cardinality = "many"` is recorded as a known wart, with the reason it stays
- [ ] The shipped `cairn.toml` mentions ordering where the statuses are

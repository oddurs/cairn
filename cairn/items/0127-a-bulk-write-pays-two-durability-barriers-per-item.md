---
id: 127
title: A bulk write pays two durability barriers per item
type: bug
status: backlog
milestone: v1.0
created: 2026-09-16
updated: 2026-09-16
priority: p2
---

## What happens

## What should happen

## Reproduction

1.
## Problem

`write_atomic` fsyncs the temporary file, renames it over the target, then
fsyncs the directory. Two barriers per item, which is exactly right for a
single write: the item is on the disk before anything claims it is.

A bulk write pays it per item. With the repeated `load_all` removed (0126), the
barriers are what remains, and they are most of the cost:

| items | `cairn set --filter` |
|---|---|
| 250 | 7.4s |
| 500 | 18.2s |
| 1000 | 49.6s |
| 5000 | over 10 minutes |

Roughly 30–50ms per item, growing with directory size, which is what a
directory fsync does as the directory fills.

## Proposal

Under one lock, a bulk change could keep the per-file fsync — each file is
whole before the rename that publishes it — and fsync the directory **once**
at the end rather than once per item.

What that gives up: after a crash mid-bulk, some renames may not have reached
the disk even though the files have. The items that survive are individually
intact; the set of them may be short. Since a bulk change is already not atomic
across items — it is a loop that can fail on the fourth of ten — this arguably
gives up a guarantee the command never offered.

What it would need before shipping:

- a measurement, not a guess, of what it actually saves
- the durability chapter updated to say so, because it currently describes the
  per-write case and would no longer be describing all of them
- a decision about whether `cairn` wants a fast path that is less durable at
  all, which is a question about the project rather than about this loop

Not done here. Deliberately left as a decision.

## Acceptance criteria

- [ ] measured, on a spinning disk as well as an SSD
- [ ] the durability chapter says what a bulk write guarantees
- [ ] `make durability` covers a crash during a bulk write

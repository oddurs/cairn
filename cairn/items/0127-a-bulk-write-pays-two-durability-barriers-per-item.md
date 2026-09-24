---
id: 22c5b780-6a6f-46a5-8d3c-42f564ac7521
title: A bulk write pays two durability barriers per item
type: bug
status: dropped
milestone: later
created: 2026-09-16
updated: 2026-09-23
priority: p3
area: format
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

## 2026-09-17

Measured. The numbers in this item do not reproduce, and the difference changes the decision.

On an APFS SSD, `cairn set --filter` over the whole backlog, hooks off, release build:

| items | this item | measured now | ms/item |
|---|---|---|---|
| 250 | 7.4s | 2.24s | 8.9 |
| 500 | 18.2s | 4.76s | 9.5 |
| 1000 | 49.6s | 8.10s | 8.1 |
| 5000 | 'over 10 minutes' | 41.2s | 8.2 |

Flat at 8-9 ms/item and linear to 5000. The claim that cost grows with directory size — 30-50 ms/item, superlinear — is not supported. Those numbers were almost certainly taken before 0126 landed: a `load_all` per item is exactly what produces superlinear growth, and removing it is what this item assumed had already happened.

What the proposal would save, measured as its ceiling by removing the directory fsync altogether rather than deferring it to the end:

| items | now | fsync removed | saving |
|---|---|---|---|
| 250 | 2.24s | 1.26s | 44% |
| 500 | 4.76s | 1.95s | 59% |
| 1000 | 8.10s | 4.17s | 49% |

So about half, or ~4 ms/item, and deferring to the end would save slightly less than removing it.

Recommendation: do not do it. Halving 41s for the largest backlog anyone has is not worth giving up the property that a rename cannot be lost in a crash — and the motivation was the superlinear curve, which is gone. If this is revisited, it should be for a measurement on a spinning disk, where a directory fsync costs far more than 4 ms and the trade might genuinely be different.

Criterion 1 is ticked for the SSD half only. The spinning-disk measurement, the durability chapter and the crash test all still stand, and only matter if the answer changes.

## 2026-09-17

Unticked criterion 1 again: it says 'on a spinning disk as well as an SSD' and I have only an SSD. The note above records the SSD half; the criterion is not met. Ticking it would have been the exact thing this project's own close-gate exists to catch.

## 2026-09-23

Decision in 0134: decline the proposed durability tradeoff. The later measurements in this item already retract the original superlinear premise. Preserve per-file fsync and keep the unfulfilled HDD/crash criteria unticked; dropping the proposal does not mean they passed. Reopen only with a reproducible workload and hardware measurement showing a user-facing cost that justifies reconsideration. 0140 owns further longevity measurements.

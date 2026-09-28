---
id: 161
uid: 0e0d2b8a-329b-4cdd-8189-bd6366cdb17f
title: Forty concurrent writers can outwait the project lock on a loaded machine
type: bug
status: done
assignee: oddurs
created: 2026-09-27
updated: 2026-09-28
closed_at: 2026-09-28
priority: p3
area: cli
effort: s
---

## What happens

`concurrent_creates_never_collide` starts 40 `cairn new` processes at once and
expects every one to succeed. On 2026-09-27, with a machine load average of
316, it failed twice in a full `make check` ("every writer got its turn"),
while the same test passed alone in 3.7 s on main and 6.5 s on a branch. The
writers queue on the project lock, whose wait gives up after 10 s
(`ACQUIRE_TIMEOUT`); forty serial writes on a loaded machine exceed that.

## Approach

Decide whether ten seconds is the right promise for many concurrent writers
(the advertised case is several agents on one backlog), and either scale the
wait with contention or have the test measure what it means — no duplicate
ids — rather than the timeout.

## Acceptance criteria

- [x] The test cannot fail on a loaded machine unless ids actually collide, or the lock's wait is documented as the limit it enforces

## 2026-09-28

Chose to fix the wait, not the test. The ten seconds were meant to catch a holder that has stopped, and were measuring a queue that was moving: forty writers each held the lock briefly, and the last in line waited longer than ten seconds in total. The deadline now restarts whenever the lock changes hands (its contents, pid and time, differ), so a waiter gives up only when one holder keeps it for ten seconds. That also covers the cross-worktree claim lock, which shares acquire_at. Starvation: a waiter behind an endless stream of writers now waits as long as the stream lasts rather than failing at ten seconds; with the queue moving that is what the user wants. Evidence: two unit tests in src/lock.rs with a short patience; the moving-queue test (13 holders x 250 ms against 1 s patience) fails on main's loop with 'gave up on a moving queue' and passes with the fix; the give-up test still gives up on a holder that never lets go. The manual's concurrency section says what the limit is.

## Result

A writer waits for the project lock as long as it keeps changing hands, and gives up when one holder has kept it ten seconds by its own recorded time, or after two minutes in all; so forty concurrent writers on a loaded machine each get their turn. Claims take their own directory's lock before the one every worktree shares, so a claim queued in one worktree no longer holds up claims in the others. Both limits are in the manual's concurrency section.

## 2026-09-28

Review (/code-review high and an independent reviewer) changed the approach. Comparing the lock file's contents measured from when a waiter first saw a holder, not from when the holder took it; it read the file twice per poll; and with no bound left, a waiter losing every create_new race (it is not a FIFO) could hang silently. Now: a waiter gives up when the holder's own recorded age (Lock::age, the since it wrote, mtime as fallback) passes ten seconds, or after two minutes in all while the lock keeps changing hands. Both reviewers also found that claims took the cross-worktree lock before their item directory's, so a claim queued in one worktree held up claims in every other; the order is now item directory first, shared second (only claim and claim_item take both, so no deadlock). Evidence: lock::tests (moving queue of 6 holders x 1 s against a 3 s limit; a holder 20 s into its hold is given up on at once; an endless queue is given up on at the overall bound), and worktrees::a_claim_queued_in_one_worktree_does_not_hold_up_another, which fails with the old order (11.7 s, FAILED) and passes with the new (4.4 s). Declined here and filed separately: two waiters breaking the same stale lock can each delete the other's fresh one (predates this change).

## 2026-09-28

Seen in CI too: windows (tier 2) on PR #119 (run 36434549444) failed concurrent_creates_never_collide with 33 of 40 writers getting their turn, on a change that did not touch the lock. So it is not only a loaded laptop: a slow runner meets the ten-second total.

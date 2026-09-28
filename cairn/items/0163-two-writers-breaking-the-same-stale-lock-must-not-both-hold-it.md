---
id: 163
uid: 42027f23-27d4-4453-9567-23fa387bd091
title: Two writers breaking the same stale lock must not both hold it
type: bug
status: backlog
milestone: v1.0
depends_on:
- 161
created: 2026-09-28
updated: 2026-09-28
priority: p2
effort: s
area: cli
---

## What happens

`Lock::acquire_within` breaks a lock older than five minutes by removing the
file and trying again. Two waiters can both read the stale lock's age. The
first removes it and creates its own. The second then removes the first's
fresh lock and creates another, so both believe they hold the lock and can
allocate the same id. The `let _ =` on that removal also discards its
error. Found reviewing 0161, and it predates that change.

## Approach

Break a stale lock by taking it over atomically, not by delete-then-create.
For example, rename it to a name unique to this waiter and check what was
renamed is the stale file just read, so only one waiter wins the takeover.
Or re-read the lock's contents immediately before removing it and remove it
only if they are unchanged, which narrows the window but does not close it.

## Acceptance criteria

- [ ] A test with two waiters racing to break one stale lock ends with exactly one holder
- [ ] A failure to remove a stale lock is reported, not discarded

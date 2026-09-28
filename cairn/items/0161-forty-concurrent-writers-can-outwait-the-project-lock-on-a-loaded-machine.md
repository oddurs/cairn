---
id: 161
uid: 0e0d2b8a-329b-4cdd-8189-bd6366cdb17f
title: Forty concurrent writers can outwait the project lock on a loaded machine
type: bug
status: backlog
created: 2026-09-27
updated: 2026-09-27
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

- [ ] The test cannot fail on a loaded machine unless ids actually collide, or the lock's wait is documented as the limit it enforces

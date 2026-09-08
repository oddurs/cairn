---
id: 106
title: A claim is a promise somebody might not keep
type: feature
status: backlog
milestone: v0.1
created: 2026-09-08
updated: 2026-09-08
priority: p0
sprint: s11
effort: m
area: agents
---

## Problem

A claim is a lock on *intent*, and it is the only lock in cairn with no timeout
and nothing that reaps it.

```
$ cairn claim 1 --as agent-one
$ cat cairn/items/0001-real-task.md
status: doing
assignee: agent-one
updated: 2026-09-08
```

That is the whole record. There is no note of *when* the claim was taken — only
`updated`, at day granularity, which any other edit also moves.

A person who stops working notices. An agent does not stop; it ceases to exist.
A session is killed, hits a limit, or the machine goes away, and item 1 is
`doing` / `agent-one` for ever:

- invisible to `cairn next`, because it is claimed
- invisible to a person, because nothing lists a claim nobody is honouring

The work rots silently, which is the worst way for work to rot.

## Proposal

**Record when.** A claim writes `claimed`, a date, alongside `assignee`. It is
its own key because `updated` means something else and every other edit moves
it.

**Make a stale claim visible, in the two places somebody is already looking.**

`cairn next` offers back a claim older than a threshold, marked as such, because
the agent asking "what should I do" is exactly the one who should be told that
this was abandoned rather than finished.

`cairn list --filter stale=true` finds them, so a person can sweep.

The threshold belongs in `cairn.toml` --- `project.claim_stale_after`, in days
--- because a project where a claim means "an afternoon" and one where it means
"this quarter" are both real, and neither should be cairn's guess. Absent, no
claim is ever stale and nothing changes for anybody who does not want this.

## What this deliberately is not

**Not a lease, and not a heartbeat.** That is a distributed-systems answer to a
filesystem problem. It would put state in the repository a person cannot read,
and it would require something to be running --- which is precisely what cannot
be assumed about the thing that stopped.

**Not automatic release.** cairn must not silently take work away from somebody
slow. Stale means *visible*, not *revoked*: `next` offers it and says why, and a
person or an agent decides.

## Acceptance criteria

- [ ] `cairn claim` records `claimed` as a date, distinct from `updated`
- [ ] `cairn release` and `cairn close` clear it
- [ ] `project.claim_stale_after` in days, absent by default and inert when absent
- [ ] `stale` is a derived field, so it works in any filter and as a column
- [ ] `cairn next` offers a stale claim back, saying how long it has been held and by whom
- [ ] Nothing is ever released automatically
- [ ] `claimed` is documented in the specification, since cairn writes it

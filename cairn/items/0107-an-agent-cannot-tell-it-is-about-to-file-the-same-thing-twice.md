---
id: 107
title: An agent cannot tell it is about to file the same thing twice
type: feature
status: backlog
milestone: v0.1
created: 2026-09-08
updated: 2026-09-08
priority: p1
sprint: s11
effort: m
area: agents
---

## Problem

```
$ cairn new "Task 1" -q   # 0008
$ cairn new "Task 1" -q   # 0009
$ cairn list -A | grep -c "Task 1"
3
```

Three identical items, filed without a murmur.

A person who files the same bug twice usually half-remembers doing it. An agent
has no such half-memory: it starts every session cold, and the backlog is not
its record but its *memory*. Filing a duplicate is therefore not an unlucky
mistake, it is the characteristic failure of an agent using this tool, and cairn
currently does nothing about it.

The cost is not the extra file. It is that the backlog stops being trustworthy
as a memory --- once there are three of something, no reader can tell which one
carries the reasoning.

## Proposal

`cairn new` and the `create_item` tool look for an item whose title is close to
the one being filed, and **say so**:

```
$ cairn new "Rate limit the API"
note: 0004 "Rate-limit the public API" and 0011 "Rate limiting" look similar
created 0042  Rate limit the API
```

Reported, not refused. Two items genuinely called the same thing is a real
situation --- two bugs with the same symptom, a chore repeated per release ---
and a tool that refused would teach agents to work around it by mangling titles,
which is worse than a duplicate.

Comparison should be cheap and boring: normalise case and punctuation, compare
word sets, report anything above a threshold. Nothing clever, nothing that needs
a model, nothing that can be wrong in an interesting way.

For the agent path it matters that this appears in the *result*, not on standard
error, because a tool result is what the model reads.

## Acceptance criteria

- [ ] `cairn new` reports near-duplicate titles and creates anyway
- [ ] `create_item` reports them in the tool result, where a model will read it
- [ ] Matching ignores case, punctuation and word order
- [ ] Closed items are included, since re-filing something already done is the same mistake
- [ ] `--quiet` suppresses the note; nothing about the exit status changes
- [ ] A test files a plausible near-duplicate and asserts the warning names both

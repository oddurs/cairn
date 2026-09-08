---
id: 111
title: What changed since I last looked
type: feature
status: backlog
milestone: v0.2
created: 2026-09-08
updated: 2026-09-08
priority: p2
sprint: s11
effort: s
area: agents
---

## Problem

There is no `--since` anywhere: not on `list`, not on `log`, not on any tool.

An agent returning to a project it worked on yesterday, or a long session
checking whether anything moved while it was working, has one option: read
everything again. That is the most expensive possible answer to *what changed*,
and it is the question a returning caller asks first.

`cairn log <id>` gives the history of one item. Nothing gives the history of the
backlog.

## Proposal

```
cairn list --since 2026-09-01
cairn list --since yesterday
```

Items whose `updated` is on or after a date. `updated` already exists and is
already maintained, so this is a filter rather than new state --- which is the
whole reason it is worth doing cheaply now.

Day granularity, because that is what `updated` records. Anything finer is a
lie, and pretending otherwise would push cairn towards timestamps in frontmatter
that people have to read.

The same on the read tools, where the saving is real.

## The honest limit, written down

`updated` moves when a field changes and not when a body does, and it says
nothing about *what* changed. `--since` is therefore a filter for "look at these
again", not an audit trail. `cairn log` is the audit trail, and it works because
git is underneath it.

If a real audit answer is ever wanted --- what changed across the backlog
between two commits --- that is `cairn log` widened to the project, and it is a
different item from this one.

## Acceptance criteria

- [ ] `--since` on `list` and `search`, accepting a date
- [ ] The same on the read tools
- [ ] `since` works as an ordinary filter comparison too, if that falls out for free
- [ ] The manual says what `updated` does and does not capture, so nobody mistakes this for an audit trail
- [ ] A date that is not one is an error naming the form it wanted

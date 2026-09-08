---
id: 110
title: A proposal should be a proposal, not prose
type: feature
status: backlog
milestone: v0.2
created: 2026-09-08
updated: 2026-09-08
priority: p2
sprint: s11
effort: l
area: agents
---

## Problem

`agent = "propose"` already exists and already works. A field or status declared
that way refuses an agent's write and tells it what to do instead:

> `claude` may propose `priority` but not set it. Add a note saying what it
> should be and why, and a person will decide.

The refusal is right. What happens next is not. The proposal becomes prose in a
body, and a person who wants to review proposals has to read every note in the
backlog to find them. There is no list, no count, nothing that says *four things
are waiting on you*.

The mechanism stops one step short of its own conclusion. The hard half --- the
permission, the enforcement, the honest refusal --- is built. The easy half, a
place for the answer to go, is missing.

## Proposal

```
$ cairn propose 12 priority=p0 --why "It blocks 0067, which blocks the release."
proposed 0012  priority: p2 -> p0
```

A proposal is a note with structure: the field, the value asked for, the value
now, who asked, when, and why. It is written into the body like any other note,
under a heading cairn can find again, because the body is where reasoning lives
and a proposal that vanished when somebody edited frontmatter would be worse
than prose.

```
$ cairn proposals
0012  priority  p2 -> p0   claude, 2 days ago
      It blocks 0067, which blocks the release.
0031  status    doing -> done   claude, today
      Criteria are met; the remaining box is about a follow-up.

$ cairn proposals --accept 12
0012  priority = p0
```

`--accept` applies it as the person running the command, and records that it was
accepted rather than silently making the change look like it was always so.

## Why this earns a command

The rule is that a command earns its place if removing it would make a real task
harder. The task is *review what agents want changed*, and today it is done by
reading every body in the backlog. A flag on `list` would not do it: a proposal
is not an item, and the thing being listed is not the same kind of thing.

`propose` could arguably be `set --propose`, and that may be the better spelling.
Worth deciding when it is built rather than now.

## What this is not

Not an approval workflow. Nothing queues, nothing blocks, nothing notifies. A
proposal is a note somebody can find, and accepting one is a normal write with a
record of where it came from.

## Acceptance criteria

- [ ] `cairn propose <id> <field>=<value> --why "..."` records field, from, to, who, when and why
- [ ] It is stored in the body, findable, and survives an unrelated frontmatter edit
- [ ] `cairn proposals` lists open ones across the backlog
- [ ] `--accept` applies one and records that it was accepted, by whom
- [ ] The protocol server exposes proposing, since an agent is the caller the feature exists for
- [ ] An agent refused by `agent = "propose"` is told about this command by name
- [ ] The spelling --- `propose` against `set --propose` --- is decided in the item before it ships

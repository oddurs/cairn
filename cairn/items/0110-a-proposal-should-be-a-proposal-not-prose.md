---
id: 110
title: A proposal should be a proposal, not prose
type: feature
status: done
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

- [x] `cairn propose <id> <field>=<value> --why "..."` records field, from, to, who, when and why
- [x] It is stored in the body, findable, and survives an unrelated frontmatter edit
- [x] `cairn proposals` lists open ones across the backlog
- [x] `--accept` applies one and records that it was accepted, by whom
- [x] The protocol server exposes proposing, since an agent is the caller the feature exists for
- [x] An agent refused by `agent = "propose"` is told about this command by name
- [x] The spelling --- `propose` against `set --propose` --- is decided in the item before it ships

## 2026-09-08

Spelling decided, before building: `cairn propose`, not `set --propose`.

The fourth rule says a command earns its line if removing it would make a real task harder. Two tasks are involved and they are different in kind. Making a proposal could indeed be a flag on `set` — it is a write that gets refused and recorded instead. But *reviewing* proposals is the task that has no home today, and it is not a listing of items: it is a listing of things somebody wants done to items. `cairn list` cannot express it and a flag on `set` cannot either.

So the surface is `cairn proposals`, which earns its line, and `cairn propose` exists because a verb whose noun is a command reads wrong the other way round — `cairn set 12 priority=p0 --propose` then `cairn proposals` names one act two ways. One pair, both spelled the same.

## 2026-09-08

Done, spelled `cairn propose` and `cairn proposals` as decided above. A proposal is a note with structure — field, from, to, who, when, why — parsed back out of the body, so it survives any frontmatter edit and several on one field accumulate rather than overwrite. `--accept` applies the most recent and records that it was accepted, so the change does not later read as though it was always so. The value is checked against the schema; the permission deliberately is not, since being refused the write is why the command exists. `propose_change` is the thirteenth protocol tool, and the refusal now names the command with the change already spelled out rather than advising a note.

---
id: 95
title: The obsolete milestone block names a command with nothing to do
type: bug
status: done
milestone: v0.1
created: 2026-09-07
updated: 2026-09-07
priority: p1
sprint: s9
effort: s
area: config
---

## Problem

A `[[milestone]]` block in a project that is already format 2 produces advice
that goes nowhere.

```
$ cairn list
cairn: cairn.toml: [[milestone]] blocks are format 1. A milestone is an item in
format 2 — run `cairn migrate`

$ cairn migrate
ok: already at format 2; nothing to migrate
```

Two commands, in sequence, contradicting each other, with nothing between them
that a person can do.

The message is right for a format 1 project and wrong for this one. Somebody
gets here by copying a milestone block out of an older README, out of another
project's file, or out of anything written before format 2 — which is to say,
out of most of what exists.

## Proposal

The message depends on which project it is talking about, because it is two
different situations wearing one sentence.

At format 1, it is a migration and the current wording is right.

At format 2 or later, the block is obsolete and there is no migration to run.
The message should say the block is no longer read, and name the two things that
replace it: a `[[type]]` for milestones and a `[[field]]` referring to it. The
project already has both if it was ever migrated, so in the common case the
remedy is to delete the block.

Better still, say what the block *would have been*, since cairn has to parse it
to complain about it: the name, and the item it would become. That turns a
refusal into an instruction.

## Acceptance criteria

- [x] At format 1 the message still names `cairn migrate`
- [x] At the current format the message says the block is obsolete and is not read
- [x] It names the `[[type]]` and `[[field]]` that replace it
- [x] Neither message sends anybody to a command with nothing to do
- [x] A test covers both, because the wrong one is right for the other project

## 2026-09-07

Done. The message now depends on which project it is talking about. At the current format it says the block is no longer read, names the milestones in it, gives the `cairn new -t milestone` line that replaces them, and reports whether the type and field that replace it are already present.

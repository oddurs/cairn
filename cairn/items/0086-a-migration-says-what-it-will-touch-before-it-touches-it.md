---
id: 86
title: A migration says what it will touch before it touches it
type: feature
status: backlog
milestone: v0.2
created: 2026-09-07
updated: 2026-09-07
priority: p1
effort: s
sprint: s9
---

## Problem

`cairn migrate --dry-run` says this:

```
  format 1 -> 2
dry run: 1 step(s) would run
```

That is the number of steps, which is the one thing nobody wants to know. The
question somebody actually has before running a migration on years of work is
**what will this touch** --- and specifically whether their item files are about
to be rewritten.

For format 1 to 2 the honest answer is remarkable and completely hidden: it
rewrites `cairn.toml`, writes one new item per milestone, and does not modify a
single existing item. Verified across seven real projects, three hundred items:
zero changed, zero missing.

Somebody deciding whether to run this deserves to be told that.

## Proposal

A migration declares its blast radius, and the dry run reports it:

```
$ cairn migrate --dry-run
  format 1 -> 2  milestones move from cairn.toml into items

  cairn.toml     rewritten
  5 item(s)      created
  0 item(s)      modified

nothing already in cairn/items will be changed.
```

The last line is the one that matters, and it should only appear when it is
true. A migration that does rewrite items says so just as plainly, and in that
case the dry run should list them.

## Why this is worth building rather than documenting

Because the answer differs per migration, and a document about migration 1 is
not an answer about migration 3. The blast radius is a property of the step, so
it belongs beside the step.

It is also the thing that makes `--dry-run` worth running at all. A dry run that
prints a step count is a dry run nobody bothers with, which means the habit is
not there when a migration finally does something dangerous.

## Acceptance criteria

- [ ] Each migration step declares what it rewrites, creates and modifies
- [ ] `--dry-run` reports it, in files rather than in steps
- [ ] "Nothing already written will be changed" appears only when true
- [ ] A migration that does rewrite items lists them under `--dry-run`
- [ ] The counts are asserted by a test against a real fixture

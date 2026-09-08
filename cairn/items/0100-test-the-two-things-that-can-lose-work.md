---
id: 100
title: Test the two things that can lose work
type: chore
status: backlog
milestone: v0.1
created: 2026-09-07
updated: 2026-09-07
priority: p0
sprint: s10
effort: l
area: durability
---

## Problem

Two pieces of code in cairn can lose work rather than merely get an answer
wrong. Neither is badly written; both are under-tested relative to what they can
destroy.

**`renumber.rs`** rewrites every filename and every reference to every id in one
pass. It has the highest blast radius per line in the program: a bug here does
not produce a wrong answer, it produces a backlog that no longer refers to
itself. Nine test invocations, plus the soak, which is the right instrument and
not a substitute for cases chosen on purpose.

**The merge driver in `git.rs`** owns conflicted content. It has already
produced one genuine data-loss defect: returning non-zero left `ours` in the
working tree with no conflict markers, so a person "resolved" a conflict they
were never shown. That class of bug recurs, because the failure looks like
success.

The common property, and the reason these belong in one pass: **interrupt it and
the project must still be coherent.** Neither has a test that interrupts it.

## Proposal

`renumber`:

- with a reference cycle already present
- with duplicate ids, which is the state it exists to repair
- with a file the process cannot write --- assert what survives
- killed after N writes: the lock is released, and the backlog is either the old
  one or the new one and never half of each
- twice in a row is a no-op
- under an `id_format` carrying a prefix

The merge driver:

- invoked with a missing ancestor
- with the same item deleted on one side and edited on the other
- with content that is not valid UTF-8
- with `cairn.toml` conflicting --- it deliberately does not handle that file, so
  assert it declines rather than trying and getting it half right

## Why one item

Because the test that matters is the same test twice, and writing it once for
both is how it gets written properly rather than twice cheaply.

## Acceptance criteria

- [ ] `renumber` is tested against a cycle, duplicate ids, and an unwritable file
- [ ] `renumber` interrupted mid-pass leaves the backlog coherent and the lock free
- [ ] `renumber` run twice is a no-op
- [ ] The merge driver is tested with a missing ancestor and a delete-versus-edit
- [ ] The merge driver declines `cairn.toml` rather than attempting it
- [ ] Neither test needs the soak to fail before it fails

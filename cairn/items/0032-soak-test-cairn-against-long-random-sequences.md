---
id: 32
title: Soak-test cairn against long random sequences
type: chore
status: done
milestone: v0.2
created: 2026-09-05
updated: 2026-09-05
priority: p1
sprint: s2
effort: s
---

## Problem

The test suite checks situations somebody thought of. Nothing exercised long
sequences of ordinary operations, where a bug needs three particular things to
happen in a particular order.

## What it found

On its first run, two things:

1. **`cairn remove` left dangling `depends_on` references.** Delete an item
   something depends on and the project failed its own `check` — an invalid
   state reached through an ordinary operation.
2. **`--plain` printed status *labels* rather than *names*,** so
   `cairn list --plain --columns status | grep doing` found nothing in a project
   whose `doing` status is labelled `in progress`. The flag exists to be piped
   into `grep` and `cut`; it now emits what a filter would accept, while the
   table keeps showing the label a person is meant to read.

The first prompted a rule that did not exist before, and now applies to both
`remove` and `milestone remove`: **a destructive command always leaves a project
that still validates, and reports whatever else it had to touch.** There is no
option to leave the wreckage, because nobody wants it.

## Acceptance criteria

- [x] Random sequences of ordinary operations, with a model checked at the end
- [x] Invariants after every step: check passes, ids unique, no temp, staged or
      lock files left behind
- [x] Deterministic: every run prints a seed that reproduces it exactly
- [x] Runs nightly in CI at ten times the per-push length

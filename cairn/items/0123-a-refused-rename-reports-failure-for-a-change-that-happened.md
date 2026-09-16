---
id: 123
title: A refused rename reports failure for a change that happened
type: bug
status: backlog
milestone: v1.0
created: 2026-09-15
updated: 2026-09-15
priority: p2
---

## What happens

## What should happen

## Reproduction

1.
## Problem

`cairn set 1 title=Taken` exits 1 with `cannot rename to .../0001-taken.md:
file already exists` — and the title *has* been changed and written. The rename
is what failed, not the set.

A reader is told the operation failed. Running it again fails again, while the
title has been correct the whole time. A script keying off the exit status
treats a completed change as not having happened.

Reproduction:

```sh
cairn init --bare --name T
cairn new 'Original title'
printf -- '---\nid: 99\ntitle: Taken\nstatus: backlog\n---\n' \
  > cairn/items/0001-taken.md
cairn set 1 title=Taken    # exit 1, but `cairn show 1` says Taken
```

The occupying file is a real item under a name belonging to another — which is
cosmetic and legal, and is the state a stopped renumber leaves behind.

Nothing is destroyed: the item in the way is untouched, and `cairn check`
reports the filename drift as a warning. The defect is entirely in what the
command says about itself.

## Proposal

Two candidates, and the choice is a policy decision rather than a fix:

1. Check the destination before saving, so the command is atomic and the error
   means what it says.
2. Keep the write, downgrade the rename failure to a warning naming both facts
   — the title changed, the file kept its name — and exit zero, since the
   filename is cosmetic and `check` already reports the drift.

The second is cheaper and matches how cairn treats filenames everywhere else.
The first is what a reader expects from a command that exits non-zero.

## Acceptance criteria

- [ ] `cairn set` does not report failure for a change it completed
- [ ] the item occupying the name is still not overwritten
- [ ] a test covers the collision, which `store.rs:168` had none for

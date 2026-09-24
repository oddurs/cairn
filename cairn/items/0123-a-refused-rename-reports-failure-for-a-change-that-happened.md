---
id: 8282d670-1dd8-4e12-bd5e-6ba793af30b9
title: A refused rename reports failure for a change that happened
type: bug
status: done
milestone: v1.0
created: 2026-09-15
updated: 2026-09-17
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

- [x] `cairn set` does not report failure for a change it completed
- [x] the item occupying the name is still not overwritten
- [x] a test covers the collision, which `store.rs:168` had none for

## 2026-09-17

Took the second option. The first — check the destination before saving — makes `set` refuse a legal metadata change because the project is in a state `check` merely warns about, which is the two commands disagreeing about how serious a filename is. This project has been consistent that the identifier lives in the frontmatter and the filename is cosmetic, so the fix follows that rather than contradicting it.

`sync_path` now returns `Renamed::{Unchanged,Moved,Blocked}` instead of a bool and an error, because the blocked case is neither. All seven callers save the item first, so every one of them had the same defect; each now chooses. `set`, `propose`, `edit` and `import` report it after the line saying what happened, the way `close` reports unticked criteria. `update_item` puts it in the MCP reply, since no agent reads stderr and refusing there would tell a model its change was rejected while the item on disk carried it. `renumber` still fails, and deliberately: it exists to make filenames and identifiers agree, so a name it cannot take means the repair did not happen.

`renaming_onto_an_existing_file_never_overwrites_it` in tests/durability.rs already covered the safety property and deliberately left the exit status alone pending this item; it now asserts the status and the retry too. The claim in this item that store.rs:168 had no test was wrong — that test landed in the meantime.

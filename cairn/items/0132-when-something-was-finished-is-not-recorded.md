---
id: 132
title: When something was finished is not recorded
type: feature
status: done
milestone: v0.1
created: 2026-09-19
updated: 2026-09-19
closed_at: 2026-09-19
priority: p1
area: format
effort: m
---

## Problem

`updated` records the last edit. That is exactly why it cannot answer *what did we
finish this week*: fixing a typo in a closed item moves it, and the week's work
changes retroactively. There is no record of when an item became done.

## Proposal

`closed_at`, a date, stamped by `cairn close` at the transition.

Not `closed`: that name is already a derived filter key resolving to a boolean,
documented in the Filters chapter and pinned by `tests/stability.rs`. Taking it
for a date would make `closed=true` compare a boolean against a date and break
the Stability chapter's promise that the filter grammar is additive only. The
stored key therefore needs a name of its own; `closed` may become the date in a
major release, with `done=true` remaining the boolean.

Additive, so no format number: the specification already says an item may gain an
optional key without a new version, and §4.1 requires readers to preserve keys
they do not recognise.

## What the hook does, measured first

The concern was that stamping might want a hook, and a hook that edits the item
which triggered it could recurse. Tested: `after-change = "cairn set
$CAIRN_ITEM_ID area=touched-by-hook -q"` on a `cairn set` exits 0 in 0.03s with
both writes landed. `hooks.rs` puts `CAIRN_NO_HOOKS=1` in the hook environment, so
a hook's own cairn call fires nothing further — always exactly one level. The
ordering is safe too: `save()`, then `drop(lock)`, then the hook, so the hook can
take the lock and the parent is already finished with the item.

So the hook is not a constraint, and it is also not the right place. A hook sees
that an item *is* done, never that it just *became* done, so a hook-based stamp
would rewrite the date on every later edit — the exact thing this exists to
prevent. It belongs in the close path, beside where `claimed` is already cleared.

## Acceptance criteria

- [x] `cairn close` stamps `closed_at` with today
- [x] Later edits, including ones that move `updated`, leave it alone
- [x] Closing something already closed does not move it
- [x] Reopening clears it, and re-closing stamps afresh
- [x] `--filter closed_at=DATE` and ordered comparison both work
- [x] `closed=true` is unchanged
- [x] Settable by hand, for an import or a project adopting cairn mid-life
- [x] `cairn check` reports a date on unfinished work, as a warning
- [x] The specification documents the key, and the second reader implements it
- [x] A golden corpus case carries it, with `updated` deliberately different

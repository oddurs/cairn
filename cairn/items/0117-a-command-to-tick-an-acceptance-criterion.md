---
id: 117
title: A command to tick an acceptance criterion
type: feature
status: done
milestone: v0.1
created: 2026-09-12
updated: 2026-09-12
priority: p1
area: cli
---

## Problem

The `feature` and `bug` templates both end in acceptance criteria, and in a
working loop they are the most-touched part of an item — you tick them as you
satisfy them. There is no command for it. `set` takes fields, `note` appends to
the body, `edit` opens `$EDITOR` and is no good in a script or for an agent.

So the thing you do most often is the one thing with no CLI, and the workaround
is rewriting the Markdown from outside the tool — which is how somebody
truncated an item to zero bytes.

Everything needed to read them already exists: `Item::criteria` parses the
boxes, `cairn show` counts them, the `criteria` filter keys query them, and
`project.require_criteria` makes `check` enforce them. What is missing is the
write.

## Proposal

```sh
cairn tick 12 3        # the third criterion
cairn tick 12 --all
cairn untick 12 3
cairn show 12 --criteria    # numbered, with state
```

One parser, shared with `Item::criteria`, so the number `tick` takes and the
count `show` prints can never disagree.

And `project.require_criteria`, which already makes `check` refuse a closed item
with unticked boxes, should make `close` refuse it too — so a project that has
asked for the gate gets it at the moment it matters rather than afterwards.

## Acceptance criteria

- [x] `cairn tick <ID> <N>...` ticks those criteria and reports the new count
- [x] `cairn tick <ID> --all` ticks every criterion
- [x] `cairn untick` is the exact inverse
- [x] Ticking what is already ticked is not an error and writes nothing
- [x] An out-of-range number is refused, naming how many there are
- [x] `cairn show <ID> --criteria` lists them numbered, and `--json` carries them
- [x] `criteria_section` is honoured by every one of these
- [x] `close` refuses an item with unticked criteria when `require_criteria` is set

## 2026-09-12

Built as `cairn tick` / `cairn untick` in `src/cmd/tick.rs`, with `show --criteria` and the MCP `tick_criteria` tool. `Item::criteria_list` is the one parser both the numbering and the count read. `require_criteria` now gates `close` as well as `check`; the default still only reports, for the reason it always gave.

## 2026-09-12

Also documented `propose`, `proposals` and `migrate` in the command reference, which had reached a release with no entry, and added `every_command_has_a_section_in_the_manual` so the next one cannot. Verified the ratchet fails when the tick section is removed.

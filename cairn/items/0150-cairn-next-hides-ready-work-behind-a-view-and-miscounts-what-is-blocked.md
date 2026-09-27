---
id: 150
uid: d2178d3e-c637-48df-be42-60fafc12e212
title: cairn next hides ready work behind a view and miscounts what is blocked
type: bug
status: done
milestone: v0.1
assignee: oddurs
created: 2026-09-26
updated: 2026-09-26
closed_at: 2026-09-26
priority: p1
area: cli
effort: m
---

## What happens

Reported from a project with 416 items, 194 open and 87 open and unblocked.
Its generated instructions start every task with `cairn next --view next`, and
its `next` view is `status=planned`. `cairn new` files work as `backlog`, so the
view matches nothing, and the diagnostics mislead about why:

```
$ cairn next --view next
nothing is ready to start
112 item(s) are blocked — `cairn next --blocked` to see what by
```

1. The blocked count ignores the view, and counts closed items and containers
   that still depend on open work. `list --filter blocked=true` said 97.
2. `--blocked` adds blocked work to the ranked list and then takes the top
   five, which are ready work. It never shows what blocks what.
3. The hint drops `--view`, so following it leaves the queue being asked about.
4. An empty view says "nothing is ready to start" while 87 items are ready
   outside it.
5. `cairn agent --view` writes instructions around a view that excludes the
   status `cairn new` assigns, and neither it nor `cairn check` says so.
6. The block's regenerate line always names `AGENTS.md`, whatever file it was
   written to.

## Approach

One scope: open, non-container work matching the view and the caller's
filters. The summary, the blocked hint and `--blocked` all count within it.
`--blocked` lists only blocked work, with its blockers; MCP `include_blocked`
keeps its meaning. Ranking is untouched.

`check` reports a selection view that leaves agents nothing while ready work
exists outside it as a note, not a warning. `check --strict` fails on
warnings, and an empty approved queue is a legitimate state — this repository's
own `next` view excludes `backlog` on purpose.

## Acceptance criteria

- [x] The summary count, `next --blocked` and `list --filter blocked=true` agree within one view
- [x] `cairn next --blocked` lists only blocked work, each with its unfinished blockers
- [x] The blocked hint repeats the view and filters it was given
- [x] An empty view says how much ready work lies outside it
- [x] `cairn agent --view` notes a view that excludes new items or has nothing ready; `cairn check` notes the second without failing `--strict`
- [x] The regenerate line names the file the block was written to
- [x] Each bug has a test that failed before the fix; existing selection tests pass

## 2026-09-26

check reports the empty selection view as a note, not a warning. The report asked for a warning, but check --strict fails on warnings and make check runs it strict; this repository's own next view excludes backlog on purpose and is empty whenever nothing is approved, so a warning would fail CI on a legitimate state. Same reasoning as the merge-driver note. The static 'excludes the status cairn new assigns' condition is only said on its own by cairn agent, when someone is choosing the view; check mentions it as the cause of an empty queue.

## 2026-09-26

On the command line, --blocked changed meaning from 'include' to 'only'. Include-then-take-five showed ready work first, which is how the reporter saw an unblocked list. MCP next_items keeps include_blocked through an arg(skip) field, so the MCP contract is unchanged. list --filter blocked=true still counts containers, which next never offers; the counts agree for any view that excludes containers, as this repository's does.

## 2026-09-26

check finds generated blocks by scanning top-level *.md files for the cairn:begin marker and reading the view back from the 'Selection uses saved view' sentence, a shared constant. A block written deeper (agent --write docs/X.md) is not seen. Verified: reproduced the report's A/B/C project by hand; next --view next now names the view, counts 1 ready outside and hints --view next --blocked, which lists C blocked by B; list --view next --filter blocked=true --count gives 1; check --strict exits 0 with the note. make check green: 20 suites, clippy -D warnings, check --render --strict.

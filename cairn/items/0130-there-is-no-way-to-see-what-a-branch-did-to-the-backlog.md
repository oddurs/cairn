---
id: 130
title: There is no way to see what a branch did to the backlog
type: feature
status: done
milestone: v1.0
created: 2026-09-19
updated: 2026-09-19
closed_at: 2026-09-19
priority: p1
area: log
effort: m
---

## Problem

`cairn log` takes one item id. There is nothing that answers *what did this branch
do to the backlog*, which is the question a reviewer has.

A pull request that touches twelve item files is reviewed by reading twelve file
diffs, each of them YAML frontmatter with a line or two changed. The summary
somebody actually wants — three closed, two new, one re-prioritised, one moved to
a different milestone — does not exist, so it is reconstructed by hand every time
or not at all.

This is the shape most collaboration in cairn takes. Items are files, so changing
them is a commit, so reviewing them is reviewing a diff. The tool that renders
items readably for a person has nothing to say about a *range* of history, which
is the one place a second person always looks.

## Proposal

Read git, like `cairn log` already does, and summarise a range:

```sh
cairn log --range main..HEAD          # what this branch did
cairn log --range HEAD~10..           # the last ten commits
cairn log --range v0.2.1..            # since the release
```

Grouped by what happened rather than by file, because that is the question:

```
  closed     0044  The rewind works
             0051  Timeline scrubbing
  new        0128  A finished project looks like it contradicts itself
  moved      0067  v1.0 -> later
  priority   0090  p2 -> p0
```

`--json` for a script, and for the thing this obviously wants to become: a
comment on a pull request saying what the backlog did, generated rather than
written.

## Why a new surface rather than a flag

CONTRIBUTING sets the bar: a new command earns its place if removing it would
make a real task *harder*, not merely different.

The task is reviewing a change to the backlog, and today it is harder — it is
twelve diffs and a mental summary. Nothing existing does it: `log` is one item,
`list` is the present state and has no notion of history, `export` is a snapshot.

It is a flag on `log` rather than a new command, because it is the same question
over a different scope and `log` already knows how to read git.

## Open questions

- Does it report on items that no longer exist, deleted in the range? It should
  say so; a removed item is a thing a reviewer wants to know about.
- A renumber touches every file in the range. The summary should say "renumbered"
  once rather than reporting fifty items as changed.
- `--patch` exists on `log` already and should keep meaning what it means.

## Acceptance criteria

- [x] `cairn log --range <a>..<b>` summarises what happened to the backlog
- [x] Grouped by what changed, not by file
- [x] `--json` carries the same, for a script or a CI comment
- [x] A renumber in the range does not read as every item changing
- [x] An item removed in the range is reported as removed
- [x] Outside a repository it says so rather than printing nothing
- [x] The existing per-item `cairn log <ID>` is unchanged

## 2026-09-19

Built as `cairn log --range`, a flag rather than a command: the same question over a different scope, and `log` already knows how to read git. It reuses `at_revision` and `describe`, so the per-item and per-range views cannot disagree about what a change is.

Two things the implementation settled that the item left open.

Items are matched by the `id` in their frontmatter, never by filename. I tried `git diff -M` first and it was worse than useless — it paired a *deleted* item with an unrelated *new* one, because two short items look alike to a similarity heuristic, and reported the pair as a retitle. Identity is not something cairn has to guess: the filename is cosmetic and the id is not.

A renumber is counted, not listed. It moves identifiers and filenames follow, so a range sees a delete and an add for each; the pair is matched on an unchanged title, status and body, so a deletion and an unrelated creation that happen to share a title are left alone.

Also: a range is a net effect, so an item created and closed inside it is `new` rather than both — and a new item that did not arrive at the initial status now says which status it did, because otherwise a reviewer cannot tell the six items that arrived done from the two that arrived open.

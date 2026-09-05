---
id: 7
title: An interactive board in the terminal
type: feature
status: backlog
milestone: later
created: 2026-09-04
updated: 2026-09-05
priority: p2
effort: l
---

## Problem

## Proposal

## Acceptance criteria

- [ ]

## Dropped, 2026-09-05

A read-only web view is right for a project with stakeholders who do not
clone. This one does not have those, and `ROADMAP.md` renders on GitHub
without anything being served. Reopen this when someone asks to see the
roadmap and cannot.

## Reframed, 2026-09-05

Not a web board. A TUI.

The original was dropped for contradicting the tool's posture — a server and a
browser, for a program whose whole claim is that it needs neither. A terminal
interface has no such problem: it is where cairn already lives, it works over
ssh, and it starts instantly.

## What it is for

`cairn board` prints. It cannot act. The daily loop — `next`, `claim`, `close` —
is already three short commands and does not need a screen. What a static print
genuinely cannot do is let you *move* through a backlog: triage forty items
without retyping an id each time, read a body without leaving the board, change
a status and see the columns rearrange.

So the scope is triage, not an application:

    j/k or arrows   move
    h/l             change column, which is to say change status
    enter           read the body
    c               claim
    x               close
    /               filter, using the same grammar as --filter
    q               quit

Read-mostly, with the few actions that remove typing. Not an editor: writing
prose belongs in `$EDITOR`, which `cairn edit` already opens.

## What it costs, honestly

This is the largest single thing anyone could add here, and the least like the
rest of it.

- A dependency on `ratatui` and an event loop, against twelve small crates today.
- It is the one part that cannot be tested the way everything else is. The suite
  drives the binary and asserts on stdout; a TUI has neither. Snapshot testing a
  frame buffer is possible and is a different discipline from every other test
  in this repository.
- It is the most "want" and the least "need" on the list. Nothing is impossible
  without it.

The argument for it anyway: adoption is this project's actual bottleneck, not
capability, and a good terminal interface is the sort of thing that makes people
try a tool. That is a real reason, and it should be named as the reason rather
than dressed up as a need.

## Sequencing

After shipping. A beautiful board for a tool nobody can install is the wrong
order, and `0035` — teaching git to resolve generated files — removes friction
that every branch hits, which this does not.

## Acceptance criteria

- [ ] Triage forty items without typing an id
- [ ] Starts in well under a second on a thousand-item backlog
- [ ] Degrades honestly on a dumb terminal rather than corrupting the screen
- [ ] Frame snapshots in the test suite, so it is not the untested corner
- [ ] `cairn board` keeps printing, unchanged: pipeable, screenshot-friendly,
      and what CI uses

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

## Trigger, 2026-09-05

Held at `later` deliberately, with a condition rather than a date.

A terminal interface breaks none of the properties a web board broke — no
server, no second source of truth, no browser. But it breaks one the web board
also would have: it is **modal**. You enter it, you are in it, you leave. This
tool's design says the backlog is not a place you visit; it is something you
query in passing, in three commands. On that axis a TUI and a web board are
closer than they look, and the terminal makes it *feel* native without making it
native.

The one thing commands genuinely serve badly is triage — forty items, deciding
what matters — which is inherently a sit-down activity. That is the whole case,
and it is a real one.

Two arguments against, worth keeping visible:

- Agents do not use terminal interfaces. This serves the human half only, and
  the human half already has the fast loop.
- It is the only component that cannot be tested the way the other hundred and
  twenty-one tests are. Frame snapshots work, and they are a second discipline
  in a project whose single discipline is a real asset.

**Build it when somebody who is not the author has triaged this backlog and
found it painful.** Until then it is adoption theatre — which is not nothing,
since adoption is the bottleneck, but it should be called that rather than
dressed up as a need.

What comes first is making the printed views worth looking at: fully in
character, cheap, testable, and it is what the README demo shows anyway.

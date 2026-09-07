---
id: 71
title: 'Say what cairn is: a record of intent that happens to be schedulable'
type: docs
status: done
milestone: v0.2
created: 2026-09-06
updated: 2026-09-07
priority: p1
effort: s
sprint: s8
---

## Problem

cairn is described as "a roadmap and issue manager". That is accurate and it
undersells the thing badly enough to shape the wrong decisions.

Look at this project's own backlog. The items run sixty to ninety lines and
carry a problem, a proposal, the costs that were weighed, and criteria somebody
else can check. `0067` is an argument that talks itself out of its own premise.
`0059` is a boundary declaration. `0070` is a rule. None of these is a ticket.

A ticket is a pointer to a conversation that happened somewhere else — a
meeting, a chat window, somebody's head — and it is worthless the moment it
closes. A cairn item is the opposite: it is worth **more** once closed, because
it is the answer to "why is it like this". That is true of an architecture
decision record and false of every issue tracker there is.

## Proposal

State it, in the README, the manual and on the site:

> A cairn item is a record of intent. It carries the reasoning that produced it,
> it lives with the code it describes, and it is worth more after it closes than
> before. Scheduling — statuses, milestones, what is ready to start — exists to
> make that record actionable, not the other way round.

## Why this is worth a document rather than a sentence

Because it decides arguments that are otherwise decided by mood, in the same way
`0059` does for the boundary.

It says why items carry reasoning rather than titles, and why a template that
seeds headings is a feature rather than friction. It says why `cairn log` was
worth building. It says why the format is specified for a horizon longer than
this program's. And it says why the answer to "can we make filing faster by
dropping the body" is no.

It also predicts what cairn should refuse. A record of intent does not need
comment threads, reactions, or a notification when somebody types. Those belong
to the conversation, and the conversation is not the record.

## What this is not

It is not a claim that cairn is only for decisions. Most items are ordinary work
and should be. It is a claim about which half is load-bearing when the two
conflict.

## Acceptance criteria

- [x] Stated in the README, the manual and the site
- [x] Says what follows from it, not only what it is
- [x] Names at least one thing cairn should therefore refuse
- [x] Short enough that somebody reads all of it

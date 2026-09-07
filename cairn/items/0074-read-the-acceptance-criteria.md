---
id: 74
title: Read the acceptance criteria
type: feature
status: backlog
milestone: v0.2
created: 2026-09-06
updated: 2026-09-06
priority: p0
effort: m
sprint: s8
---

## Problem

Every item in this project has an `## Acceptance criteria` section full of
`- [ ]` boxes. Nothing reads them.

The only checkbox code in the program renders roadmap lines from an item's
*status*. The criteria are prose the tool cannot see, so an item can be closed
with every box unticked and `cairn check --strict` is perfectly happy.

That is the gap between what cairn claims about itself — that items carry the
thinking, including how you will know when it is done — and what it can
actually verify.

## Why this matters most for agents

An agent closing an item is the moment a person most wants a second opinion. A
ticked box is a claim the agent made and can be asked about; an unticked one is
a question. Machine-checkable criteria turn "is this done" from a judgement into
a comparison, which is exactly the kind of work to hand to a program.

It also improves what agents write. A criterion nobody can tick is a criterion
badly written, and having the tool count them makes that visible on the day it
is written rather than at review.

## Proposal

Parse `- [ ]` and `- [x]` from the body — from a configured section if
`cairn.toml` names one, otherwise from anywhere.

- `cairn check` warns when an item in a `done` category has unticked criteria.
- `cairn show` and `cairn next` report `3/5`.
- `criteria` and `criteria_done` become filterable, so
  `--filter 'category=done,criteria_done<criteria'` finds the ones that slipped.
- The MCP `close_item` tool reports what is unticked in its reply rather than
  refusing, because the agent may be right and the criteria stale.

## What this must not become

Not a gate. An item can be legitimately closed with an unticked box — the world
changed, the criterion was wrong — and a tool that refuses will teach people to
write criteria they can tick rather than criteria that are true. A warning says
"look at this", which is all that is wanted.

## Acceptance criteria

- [ ] Boxes are counted from the body, wherever they are
- [ ] `check` warns on unticked criteria in a closed item, and does not fail
- [ ] `criteria` and `criteria_done` are filterable and sortable
- [ ] Closing over MCP reports what remains unticked
- [ ] A body with no boxes produces no warnings and no noise
- [ ] Nothing is written to the file; this reads what is already there

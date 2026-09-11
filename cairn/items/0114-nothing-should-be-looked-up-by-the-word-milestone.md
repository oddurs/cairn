---
id: 114
title: Nothing should be looked up by the word milestone
type: feature
status: done
milestone: v0.2
depends_on:
- 113
created: 2026-09-11
updated: 2026-09-11
priority: p1
sprint: s12
effort: m
area: schema
---

## Problem

`0094` established that cairn looks up `milestone` by that literal string — in
the reference machinery, in `list`, `show`, `import` and `init`, and as the
default `render.group_by`. Rename the field and the roadmap goes blank.

`0094` fixed the *symptom*: `roadmap` and `board` now say why they have nothing
to draw, and `check` reports a `group_by` naming a field that does not exist. It
could not fix the cause, because the cause is that container-ness is derived
from a ref field's target and the only way to find the container was to know its
name.

`0089` decided separately not to build a role mechanism — a way to declare that
a field named `release` plays the milestone role — on the grounds that it costs
grep-ability and nobody had asked. That decision named a reversing condition: a
real project whose word is wrong enough that its people keep having to
translate.

Both items are about the same thing from opposite ends, and `0113` dissolves it.

## Proposal

Once a type declares that it groups work, **nothing needs to look for the word
`milestone`**. The roadmap groups by the type that groups. Call it `release`, or
`version`, or `sprint`, and everything keeps working — because cairn is looking
for a structural fact rather than a name.

So:

- `render.group_by` defaults to the single-valued container type, whatever it is
  called, rather than to the literal `milestone`.
- `refs::Milestones` stops being about milestones. It is *the containers of the
  single-valued type*, and its name should say so.
- `MILESTONE_TYPE` and `MILESTONE_FIELD` are deleted.
- `cairn init` scaffolds a type called `milestone` because that is the word most
  projects want — a default, not a promise, which is exactly the distinction
  `0089` asked for and could not previously have.

## What this settles

`0089` can close. Not because roles were built, but because the question it was
about — may a project use its own word — is answered yes for the one field where
it mattered, without a mechanism, without a lookup table, and without costing
anybody `grep 'milestone:'` who keeps the default.

The general question `0089` raised, whether *every* documented key should be
renameable, stays answered: no. `status`, `assignee`, `depends_on` remain
promises. The difference is that those are genuinely universal, and the name of
the thing work is filed under never was.

## Acceptance criteria

- [x] No lookup by the literal string `milestone` anywhere in the source
- [x] `render.group_by` defaults to the single-valued container type
- [x] A project whose container type is called `release` has a working roadmap,
      board, `next` and `check`, with no configuration beyond the type
- [x] `cairn init` still scaffolds `milestone`, as a default
- [x] `0089` is closed with this as the reason, and the manual's roles section
      records that the question was dissolved rather than decided
- [x] `0094`'s tests still pass: a project with nothing to group by still says so

## 2026-09-11

Done. Nothing decides anything by the literal string `milestone`: `schedule_type()` finds the single-valued grouping type and `schedule_of()` reads the field of that name. A project whose type is called `release` has a working roadmap, board, next and check with no configuration beyond the type. `cairn init` still scaffolds `milestone` — a default, which is exactly the distinction 0089 asked for and could not previously have.

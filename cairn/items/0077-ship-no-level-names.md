---
id: 77
title: Ship no level names
type: docs
status: done
milestone: v1.0
depends_on:
- 72
created: 2026-09-06
updated: 2026-09-07
priority: p1
effort: s
sprint: s8
---

## Problem

Once composition exists (`0072`), the request that follows within a day is a
name for the levels — initiative, epic, story, task — so that a big item can be
declared big.

cairn should ship none of them, and this item exists so that is a decision
rather than an omission somebody quietly fixes.

## The argument

Naming a level is where every tool in this category starts lying to its users.

The moment `epic` exists, people argue about whether something is an epic. The
argument has no answer in it, because the levels are not facts about the work;
they are a vocabulary imposed on it, and the vocabulary fits some projects and
not others. Then the tool's own name for a thing and the team's name for it
diverge, and everybody carries a translation table in their head.

The levels are also unstable in a way the tool cannot see. Something filed as an
epic turns out to be an afternoon; something filed as a task turns out to need
six items beneath it. The label does not update, and after a quarter the level
field is decoration.

Under `0072` and `0073` none of it is needed. A project that wants initiatives
declares `type = "initiative"`, which is one line of its own configuration and
carries no opinion from cairn at all. Size is a fact about the graph rather than
a claim in a field: `descendants` cannot be stale.

So the position is: **cairn ships the ability to express a hierarchy, and no
hierarchy.**

## What would reverse this

Somebody demonstrating a behaviour that genuinely needs a declared level rather
than a derived one — a query, a view, a rollup that cannot be written against
`part_of` and `descendants`. That would be a fact rather than a preference, and
it would settle it.

Wanting the word is not that. The word is available: it is a type name, and it
belongs to the project rather than to this program.

## Acceptance criteria

- [x] The manual states it as a position, with the reasoning
- [x] It says what a project should do instead, in one line of configuration
- [x] It names what would reverse it
- [x] A feature request asking for epics can be answered by pointing at it

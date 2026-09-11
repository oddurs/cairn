---
id: 113
title: A type declares that it groups work
type: feature
status: done
milestone: v0.2
created: 2026-09-11
updated: 2026-09-11
priority: p0
sprint: s12
effort: xl
area: schema
---

## Problem

Everything is an item; a type labels it. But some types are structurally
different — a milestone is absent from `cairn next`, from the board, from an
ordinary `list`, and accumulates rollup progress instead of having any of its
own. Five commands treat it as another kind of thing.

cairn calls that a **container**, and the word appears seven times in the manual,
in code comments, and nowhere a user will ever see:

```
$ cairn config
types
  feature
  bug
  chore
  docs
  milestone  a release, or whatever this project ships
```

Nothing says `milestone` is not like `bug`.

Worse, it is not declared. It is **derived, from the wrong end**: a type is a
container because some ref field names it as a `target`. So to learn that
milestones are special you must find a `[[field]]`, read its `target`, and know
what that implies. Adding a ref field silently changes an unrelated type's
behaviour.

And it costs eleven lines that say "milestone" four times:

```toml
[[type]]
name = "milestone"
description = "a release, or whatever this project ships"

[[field]]
name = "milestone"
kind = "ref"
target = "milestone"
by = "key"
rollup = true
inverse = "scheduled"
```

Three relations are built from one mechanism and mean different things ---
`depends_on` sequences, `milestone` schedules, `part_of` composes --- with the
differences encoded as a bitfield across `cardinality`, `by`, `rollup`,
`acyclic` and whether `target` is specific. `FieldDef` carries fourteen knobs,
six of which exist only for refs, and for the commonest ref all six are forced
to the same values every time.

## Proposal

**A type declares that it groups work.**

```toml
[[type]]
name = "milestone"
groups = "one"              # work belongs to at most one of these
inverse = "scheduled"
description = "a release, or whatever this project ships"
```

That one block does everything the eleven lines did: makes the type a container,
creates the `milestone:` field on items addressed by key, implies rollup and
acyclicity, and names the inverse. `groups = "many"` gives a type whose field
takes several — an `epic`, a theme, whatever the project calls it.

Three relations become two:

- **needs** --- sequencing, blocks `next`. Built in, undeclarable, universal.
- **within** --- composition, rolls up. Declared on the container type, and the
  field is named after the type.

`milestone` and `part_of` were never two relations. They are one relation whose
cardinality is a property of the *target type*: "an item ships in exactly one
release" is a fact about releases, not a second concept.

`kind = "ref"` survives for genuine general references --- a `related` field
pointing anywhere --- keeps its knobs, and becomes rare rather than
load-bearing.

## Why this is worth a format number

**No item file changes.** `milestone: v0.1` is byte-identical before and after.
This is format 2's property exactly: the migration rewrites `cairn.toml` and
touches nothing in the item directory, which is what made that bump cheap enough
to be worth making.

What it buys:

- Container-ness becomes **declared and visible**, in the file and in
  `cairn config`.
- `type` stops doing two jobs. It is a label; `groups` is the structural fact.
- `is_container` reads a flag instead of scanning ref targets. The
  `board --group-by milestone` defect — which silently dropped every scheduled
  item off the board — was a miscomputed `is_container`, and that class becomes
  impossible.
- Six knobs leave the common path.

## What this deliberately is not

**Not roles, and not field aliases.** `0089` decided against a rename mechanism
and this removes the need for one rather than reopening it (see `0114`).

**Not a merge of `needs` into `within`.** Sequencing and composition answer
different questions, and collapsing them would lose the constraint that makes a
dependency mean anything.

**Not a type hierarchy.** Types stay flat. `groups` is one bit, not a tree.

**Not a first-class "epic".** An epic is a type that groups, which is the whole
point of declaring it rather than building it in.

## Acceptance criteria

- [x] `[[type]] groups = "one" | "many"` declares a container and its field
- [x] The field is named after the type, addressed by key, rollup and acyclic implied
- [x] `inverse` moves to the type
- [x] `is_container` reads the declaration rather than scanning ref targets
- [x] `cairn config` shows which types group work, and `--json` carries it
- [x] `kind = "ref"` still works for a general reference to `*`
- [x] Format 3, with a migration that rewrites `cairn.toml` and touches no item file
- [x] The migration is verified against every project using cairn today
- [x] `tests/golden/format-2/` is frozen with a digest, as format 1 was

## 2026-09-11

Done, format 3. A type says `groups = "one"` or `"many"`; that one declaration creates the field, addressed by key, with rollup and acyclicity implied — eleven lines that said 'milestone' four times became four. `is_container` reads the flag rather than scanning ref targets, so the class of bug that emptied the board is structurally impossible. `cairn config` shows which types group work. The migration rewrites cairn.toml and touches no item file: verified byte-identical across unifont, dirk and milky-xl, 294 items.

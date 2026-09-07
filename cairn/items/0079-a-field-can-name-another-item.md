---
id: 79
title: A field can name another item
type: feature
status: done
milestone: v1.0
created: 2026-09-07
updated: 2026-09-07
priority: p0
effort: m
sprint: s8
---

## Problem

`cairn.toml` describes an item with two unrelated mechanisms and a hole between
them.

`[[field]]` declares attributes: a name, a kind, maybe an enum of permitted
values. Fields are filterable, sortable, showable as columns, and can have
defaults and be required.

`depends_on` is not a field. It is a graph edge, hardcoded, undeclarable, with
its own parser, its own cycle check and its own repair path in `renumber`.

And there is a third thing with no mechanism at all: a value that names
something with its own existence. `milestone = "v0.1"` looks like an enum, but
`v0.1` is not a string — it has a due date, a description, a reason for that
date, and a history of the date moving. Today all of that lives in
`cairn.toml`, which cannot hold a body and cannot be asked when something
changed.

`0072` proposed a second table, `[[relation]]`, for declarable edges. Building
it would have been a mistake, and this item says why and what to do instead.

## The observation

A ref needs a target type, a cycle check, an inverse and a rollup. That is the
whole of what a relation needs. Two tables means implementing it twice and
explaining it twice, and an agent learning two vocabularies for one idea.

They are the same thing. The only differences are **cardinality** — one value or
many — and **how the value addresses its target**.

    milestone: v0.1        one value, addressed by a human-readable key
    depends_on: [12, 13]   many values, addressed by identifier
    part_of: [7]           many values, addressed by identifier

So: one table. Everything in an item's frontmatter is a field, and a field's
kind may be `ref`, meaning its values name other items.

## Proposal

```toml
[[field]]
name        = "milestone"
kind        = "ref"
target      = "milestone"    # values name an item of this type
cardinality = "one"          # default
by          = "key"          # or "id" (default)
rollup      = true           # contributes to progress
inverse     = "scheduled"    # optional; how to ask the question backwards

[[field]]
name        = "depends_on"
kind        = "ref"
target      = "*"            # any item
cardinality = "many"
acyclic     = true
ordering    = true           # feeds ready, blocked and `cairn next`
inverse     = "blocks"
```

`depends_on` ships as exactly that declaration, so it means what it already
means and no file changes. It stops being special, which was `0072`'s point.

### Addressing, and why `key` exists

`by = "id"` points at the unsigned integer, which is what identity is
(`0062`). `by = "key"` points at a short human handle:

```yaml
---
id: 42
type: milestone
key: v0.1
title: Usable in anger
due: 2026-10-15
---
```

`key` is **not** a second identity. Identity is the integer; the key is a
handle, unique within a type, that a person reads and types. The precedent is
already in the schema one level down: a status has a `name` for machines and a
`label` for people. This is that, one level up.

Without it, `milestone: 42` is what an item file says, and the readability that
makes this format worth having is gone for the sake of internal tidiness.

Three rules make it unambiguous:

- A key is unique among items of its type. `check` reports a collision.
- A key **must not** parse as an identifier under the project's `id_format`, so
  `milestone: 0042` can never be two things. Same rule, same reason, as the
  prefix restriction in `0062`.
- A `by = "key"` field resolves **only** by key. Never by id, never by title. An
  unresolvable value is an error naming the keys that exist.

### Changing a key

Renaming a milestone orphans every item pointing at it. That is true today of
renaming one in `cairn.toml` — the difference is that items invite editing, so
it will happen more.

Changing a key rewrites the references, under one lock, in the shape `renumber`
already uses for identifiers: print what will change, do it atomically, and say
so. It is a rename, not an ordinary assignment, and it should not be silent.

### What a ref target is not

An item that is the target of a **named** type (`target = "milestone"`, not
`target = "*"`) is a container. It does not appear in `cairn next`, because
`next` answers what can be started and a container cannot be. It does not
appear on the board as work.

This also settles a problem that would otherwise arrive with it: milestone items
that depend on each other would read as blocked, and blocked containers would
crowd out real work. Containers are not in `next`, so they cannot.

If somebody produces a type that is genuinely both a container and startable
work, this becomes a flag on the type. Nobody has, so it does not.

### One derivation for progress

A field marked `rollup = true` contributes to progress the same way whatever the
cardinality or addressing. Progress is the proportion of items pointing at this
one, through any rollup field, that are in a `done` category. One rule, one
implementation, however many fields point in. `0073` derives it.

## What this does not change

Nothing on disk. `depends_on` is declared as what it already is. `kind = "ref"`,
`target`, `cardinality`, `by`, `acyclic`, `ordering`, `rollup`, `inverse` and
`key` are all new optional keys.

**Format 1. No migration.** Every existing project keeps working having changed
nothing, and gains the mechanism.

`0078` spends a format number, and it spends it on removing `[[milestone]]` from
the configuration — not on this.

## Acceptance criteria

- [x] `kind = "ref"` exists, with `target`, `cardinality`, `by` and `inverse`
- [x] `depends_on` is declared rather than hardcoded, and behaves identically
- [x] `acyclic` and `ordering` are properties of a field, not of one built-in
- [x] A key is unique within its type, and cannot parse as an identifier
- [x] An unresolvable ref is an error naming the keys that exist
- [x] Changing a key rewrites the references atomically, and says what it did
- [x] Items of a named target type are absent from `next` and from the board
- [x] Refs are filterable, sortable and showable as columns, like any field
- [x] `cairn agent` and `get_schema` describe refs in one vocabulary, not two
- [x] A project that declares nothing behaves exactly as it does today
- [x] No format bump

## 2026-09-07: built

One caveat worth recording rather than leaving to be discovered.

`depends_on` is *described* through the general mechanism — `cairn config
--json` reports it as a many-valued, id-addressed, acyclic reference alongside
every declared field — but it is still *stored* on `Meta` as a typed
`Vec<u32>` rather than in the generic bag. That is deliberate: the key is
documented in the specification as a sequence of unsigned integers, and typing
it is what keeps `renumber`, import and the interchange document honest.

So the schema has one vocabulary and the storage has two paths. The second is
invisible to anybody reading or writing a file, and collapsing it would mean
untyping a specified key for tidiness. Worth knowing before somebody assumes
the generic path covers everything.

`rollup` is declared and reported in the schema but nothing derives from it yet.
`0073` is what reads it.

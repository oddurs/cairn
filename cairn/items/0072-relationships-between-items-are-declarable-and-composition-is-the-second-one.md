---
id: 72
title: Relationships between items are declarable, and composition is the second one
type: feature
status: backlog
milestone: v1.0
depends_on:
- 79
created: 2026-09-06
updated: 2026-09-07
priority: p0
effort: l
sprint: s8
---

## Problem

`cairn.toml` declares the item types, the statuses they move through, the custom
fields, the milestones and the saved views. The claim on the front page is that
the schema is yours and nothing is hardcoded.

There is exactly one relationship between items, `depends_on`, and it is not
declarable, not redefinable, and has no siblings. It is the single place the
central claim is untrue.

It is also the only relationship, which means composition — the thing everybody
asks for within a week — has nowhere to live. "Ship OAuth" can currently be a
label or a milestone. A label has no progress and no body. A milestone is
date-shaped and lives in configuration. Neither is what anybody means.

## The design question

The obvious answer is a `parent` field, and it is the wrong one.

A tree has exactly one path to each node, and real work does not. Is the OAuth
item under the Auth initiative or under the Q3 security push? Both — and a tree
makes you choose, then makes you wrong later. Everybody who has used a tracker
with epics has moved one.

Worse here specifically: a tree destroys the merge story. `parent` is a scalar,
so two branches reparenting the same item is a genuine conflict with no derived
resolution, and cairn's entire merge argument is that the contested files are
derived and can be re-derived. A set of edges unions cleanly. A parent pointer
does not.

And agents are poor at hierarchies and good at edges. An agent asked to file
something "in the right place" invents a place.

`depends_on` is already a directed acyclic edge. Composition is also a directed
acyclic edge. The missing thing is not a tree; it is a second **kind** of edge,
and the ability to declare kinds.

## Proposal

```toml
[[relation]]
name = "depends_on"
inverse = "blocks"
acyclic = true
ordering = true          # feeds ready, blocked, and `cairn next`

[[relation]]
name = "part_of"
inverse = "contains"
acyclic = true
rollup = true            # feeds progress
```

Both ship as defaults, so an existing project is unchanged and a new one gets
composition without configuring anything. `part_of` is many-to-many on purpose:
an item belongs to OAuth and to Q3 security at once.

Depth is unbounded. A limit is a decision that will be wrong for somebody, and
the failure it guards against — a taxonomy where a plan was wanted — is better
handled as a smell than as an error. `cairn check` warns past a chain of about
four, in the machinery it already has for filenames.

## Two constraints this must not break

**`cairn next` stays flat.** It answers "what can I start", and an agent with a
context budget must never traverse a graph to find work.

**Filing stays free.** `cairn new` must never require a parent. Structure is
added afterwards, by a second command, which is also what keeps merges cheap.

## Compatibility

`part_of` is a new optional key in the frontmatter and `[[relation]]` is a new
optional table, so this is additive: format 1, no migration, existing files
unchanged. Making it declarable does not change what `depends_on` means.

## Acceptance criteria

- [ ] Relations are declared in `cairn.toml`, `depends_on` among them
- [ ] `part_of` ships as a default, many-to-many, cycle-checked on every write
- [ ] An inverse is queryable without being stored twice
- [ ] `cairn next` output is unchanged for a project that uses no relations
- [ ] `cairn new` still requires nothing but a title
- [ ] Two branches that both link the same item merge without a conflict
- [ ] `check` warns on a chain deeper than four, and does not fail
- [ ] No format bump

## 2026-09-07: the `[[relation]]` table is superseded

Filing `0079` showed that a second table was the wrong shape.

A ref needs a target type, a cycle check, an inverse and a rollup — which is the
entire feature set a relation needs. Two tables means implementing that twice,
documenting it twice, and handing an agent two vocabularies for one idea.

They differ only in cardinality and in how a value addresses its target, so they
are one thing. `0079` collapses them: everything in an item's frontmatter is a
field, and a field's kind may be `ref`.

What this item asks for is unchanged, and is now a two-line declaration rather
than a new subsystem:

```toml
[[field]]
name        = "part_of"
kind        = "ref"
target      = "*"
cardinality = "many"
acyclic     = true
rollup      = true
inverse     = "contains"
```

Everything above about trees, merge behaviour and unbounded depth stands. The
argument against a `parent` scalar is unaffected: this is still a set of edges,
and sets union at a merge.

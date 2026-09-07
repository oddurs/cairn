---
id: 80
title: Merge sequence fields by union
type: feature
status: done
milestone: v1.0
depends_on:
- 72
created: 2026-09-07
updated: 2026-09-07
priority: p1
effort: m
sprint: s8
---

## Problem

`0072` asserted that two branches linking the same item would merge without a
conflict, because edge sets union. Testing it showed otherwise, and the test now
says so.

git merges text. Two branches each appending to the same YAML sequence — beside
an `updated` stamp both of them also touched — is an ordinary textual conflict.
`depends_on` has always had this property and composition inherits it, so this
is a gap that predates both.

What is still true is the part the argument rested on: a set has a resolution
that keeps both intentions, while a scalar `parent` does not — one side simply
loses and nobody can tell which was meant. The conflict is a merge somebody can
finish rather than a decision the format destroyed. That is worth a great deal
and is not the same as no conflict.

## Proposal

cairn already installs a merge driver, for generated files whose answer is to
re-derive them. Item files are different: they are the source. But a *sequence*
field has an answer too, and it is the union.

Extend the driver to resolve an item file itself when the only differences are:

- a sequence-valued field, where the union of both sides is taken
- `updated`, where the later date wins

Anything else stays a conflict, because anything else is two people saying
different things about one fact, which is not a merge cairn should guess at.

## What to be careful about

The union must be order-stable, or the file churns on every merge and a
`--check` in CI fails for no reason.

`depends_on` gets the same treatment, which fixes a wart nobody had filed:
today two branches adding a dependency to the same item conflict, and the
resolution is always the union.

And it must be genuinely conservative. A driver that resolves too much loses an
edit somebody meant, and the whole reason to trust the current one is that it
only ever touches files it can re-derive from scratch.

## Acceptance criteria

- [x] Two branches adding to the same sequence field merge, keeping both
- [x] The union is order-stable, so an unchanged item is not a diff
- [x] A conflict in any scalar field is still a conflict
- [x] `updated` resolves to the later date, and nothing else does
- [x] A test performs a real merge for `depends_on` and for a declared ref
- [x] The manual says exactly what the driver will and will not resolve

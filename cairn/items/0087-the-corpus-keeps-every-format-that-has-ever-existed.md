---
id: 87
title: The corpus keeps every format that has ever existed
type: chore
status: done
milestone: v1.0
depends_on:
- 86
created: 2026-09-07
updated: 2026-09-07
priority: p0
effort: m
sprint: s9
---

## Problem

`tests/golden` pins how cairn parses item files, and it is the best test in the
project. Every case in it is a **current** file.

So nothing checks the thing that actually breaks people: that a cairn built next
year can still read a file written this year. The corpus tests today against
today.

That gap is invisible until it is expensive. A key quietly changing meaning
between formats would pass every test here and be discovered by somebody whose
backlog stopped making sense.

## Proposal

The corpus gains a directory per format, and every one of them is checked on
every run.

```
tests/golden/
  format-1/     items as format 1 wrote them, with their expectations
  format-2/     ...
  current -> format-2
```

Two properties, both asserted:

**Every format still parses.** A file written by any cairn that ever existed is
read correctly by the current one --- values, not merely absence of error.

**Migration is lossless in the way it claims.** Running `cairn migrate` over a
copy of an older format's corpus produces the current format's expectations, and
touches nothing it said it would not touch (`0086`).

The corpus for a format is frozen the day that format stops being current.
Nobody edits `format-1/` afterwards; if it is wrong, it was wrong then, and that
is what a reader in ten years will meet.

## Why this rather than more example tests

Because it is the only test that can fail for the right reason years later. The
suite currently proves cairn is self-consistent. This proves cairn is consistent
with **its own past**, which is the promise the format number is making on its
behalf.

The second reader (`spec/reader.py`) should run against every format too. A
specification that only describes the current one is not a specification of the
format; it is a specification of the moment.

## Acceptance criteria

- [x] A directory per format, each with items and their expectations
- [x] Every format parses correctly under the current cairn, checked in CI
- [x] Migrating an older corpus produces the current expectations exactly
- [x] An older corpus is frozen: a test fails if one is edited after its format
      stopped being current
- [x] `spec/conformance.py` runs against every format, not only the newest
- [x] Adding a format without adding its corpus fails the build

## 2026-09-07

Done. tests/golden/format-1/ holds twelve cases as format 1 wrote them. Four things are checked: every format still parses under the current cairn, migrating an older corpus produces the current expectations exactly, each frozen corpus matches a committed FNV-1a digest, and a format below CURRENT_FORMAT without a corpus fails the build. spec/conformance.py runs the second reader over every format.

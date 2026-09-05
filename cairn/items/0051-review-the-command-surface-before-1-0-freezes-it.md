---
id: 51
title: Review the command surface before 1.0 freezes it
type: chore
status: backlog
milestone: v1.0
created: 2026-09-05
updated: 2026-09-05
priority: p0
effort: m
---

## Problem

Twenty-seven subcommands, written over a handful of days, none of them ever
reviewed as a set. After 1.0 every one of them is a promise, and removing one
is a major release.

Some were written speculatively and the evidence is now in:

- `migrate` migrates nothing, and by design will not until there is a format 2.
  It exists so the path is tested. Is a command in `--help` the right home for
  that, or should it be a hidden verb until it has work to do?
- `renumber --compact` renumbers a whole project into a gapless sequence. It has
  never been needed outside its own test.
- `board`, `roadmap` and `list` are three views of one query. That is defensible
  — they answer different questions — but it should be a decision rather than an
  accident.
- `export` and `import --from json` are one round trip split across two verbs
  with different flags.

## Proposal

Go through all twenty-seven and put each in one of three groups: keep, hide
from `--help` while remaining callable, or remove before the surface is frozen.
Write the reasoning down — an item nobody can justify is a candidate for
removal, and "somebody might want it" is not a justification.

The bar: a command earns its place if removing it would make a real task harder,
not merely different.

## Acceptance criteria

- [ ] Every command has a written justification or a removal
- [ ] `--help` shows what a person needs; internal verbs are hidden
- [ ] Nothing is removed after the review without a major release
- [ ] The manual and the site agree with the result

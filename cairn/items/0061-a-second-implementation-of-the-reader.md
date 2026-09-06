---
id: 61
title: A second implementation of the reader
type: feature
status: backlog
milestone: v1.0
created: 2026-09-06
updated: 2026-09-06
priority: p1
effort: m
---

## Problem

`spec/README.md` claims a reader can be implemented from it alone, without
consulting cairn's source. Nobody has tried. Until somebody does, that is a
claim rather than a fact, and every specification that has never been
implemented twice contains at least one thing that is only true because of how
the original happens to work.

This matters more than usual here, because the format — not the program — is
what anything else builds on. If cairn is git, the format is the object model,
and an object model nobody else has implemented is not really an object model.

## Proposal

A small reader in another language — fifty or so lines of Python — that parses
an item and reports its fields. Run it against `tests/golden` in continuous
integration and compare its answers to the committed expectations. When the
corpus and the reference reader disagree, one of them is wrong, and finding out
which is exactly the point.

It is written from the specification, not from `src/item.rs`. Reading the Rust
would defeat the purpose entirely.

## Licensing

Under the specification's terms rather than the program's. The point is that
people copy it into their own tools, including proprietary ones — a GPL
reference implementation would discourage precisely the thing it exists to
encourage.

## Acceptance criteria

- [ ] Written from the specification alone, and says so
- [ ] Agrees with `tests/golden` on every case, checked in CI
- [ ] Every disagreement resolved by fixing the specification or the corpus, and
      the fix recorded
- [ ] Permissively licensed, unlike the program
- [ ] Short enough that reading it is a reasonable way to learn the format

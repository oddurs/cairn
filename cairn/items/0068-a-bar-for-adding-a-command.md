---
id: 68
title: A bar for adding a command
type: chore
status: backlog
milestone: v0.2
created: 2026-09-06
updated: 2026-09-06
priority: p1
sprint: s7
effort: s
---

## Problem

Twenty-eight commands, written over three days, each locally justified and
collectively more than anybody needs. `0051` reviews them retrospectively, which
is the expensive way — pruning after the fact is much harder than not adding.

There was never a bar. Every command seemed reasonable at the moment it was
written, which is how surfaces grow.

## Proposal

One sentence in `CONTRIBUTING.md`, applied before the code is written rather
than at review:

> A new command earns its place if removing it would make a real task *harder*,
> not merely different. If an existing command with a flag would do, that is the
> answer.

And a second, narrower rule that would have caught most of the actual
accumulation:

> A command that exists to be tested rather than used is hidden from `--help`.

`migrate` is the example: it exists so the migration path is exercised before it
is needed, which is a good reason for it to exist and a poor reason for it to
occupy a line in the help output.

## Acceptance criteria

- [ ] Both rules in CONTRIBUTING, above the section on tests
- [ ] The pull request template asks which one applies
- [ ] `0051` applies the same bar retrospectively

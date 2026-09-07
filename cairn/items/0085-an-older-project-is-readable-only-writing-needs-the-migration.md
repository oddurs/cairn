---
id: 85
title: An older project is readable; only writing needs the migration
type: bug
status: done
milestone: v0.2
created: 2026-09-07
updated: 2026-09-07
priority: p0
effort: s
sprint: s9
---

## Problem

Bumping the format to 2 made seven projects on one laptop stop working
entirely. Not their writes --- everything. `cairn list`, `cairn next`,
`cairn roadmap` and `cairn check` all refuse a format 1 project and say to run
`cairn migrate`.

Nothing about those projects was unreadable. Format 2 moved milestones out of
the configuration and changed no item file at all: the items were fine, the
schema was fine, and the only thing a reader could not do was write a milestone
where the configuration still expected one.

This is a self-inflicted break, and it is the kind that teaches people a tool is
not safe to depend on.

## What went wrong

§8 of the specification says:

> A reader encountering a version it **does not understand** must refuse the
> project and say so.

A cairn that writes format 2 understands format 1 perfectly well. The rule is
about a version from the *future*, where reading on a best-effort basis means
misreading data nobody can predict. It says nothing about a version from the
past, and I applied it to both.

## Proposal

Refuse to **write**, not to read.

- Every read-only command works on an older project: `list`, `show`, `next`,
  `search`, `board`, `roadmap`, `log`, `export`, `config`, `check`, `agent`,
  and the read half of the Model Context Protocol server.
- Anything that writes refuses, and names the migration.
- A read on an older project prints one line on standard error saying the
  project is behind and what to run. On standard error, so a script reading
  `--json` is unaffected.
- A version from the *future* is still refused outright, for both reading and
  writing. That part of §8 is right and stays.

`check` is deliberately in the read list. Somebody trying to find out whether
their project is healthy should not be told to change it first.

## The rule underneath

**A format bump may cost somebody a command. It must never cost them their
data, and it must never cost them the ability to look.**

That is worth stating in the manual next to the compatibility rules, because it
is the promise that makes a version number tolerable rather than a threat.

## Acceptance criteria

- [x] Every read-only command works against a format 1 project
- [x] Every writing command refuses, naming `cairn migrate`
- [x] The notice goes to standard error, so `--json` is unaffected
- [x] A newer format is still refused for reading as well as writing
- [x] A test drives a real format 1 project through every read command
- [x] The manual states the rule, not only the behaviour

## 2026-09-07

Done. Reading an older project works because the format check moved from load to the write lock: every read command is unaffected, and the one line saying the project is behind goes to standard error so --json is untouched. A newer format is still refused outright.

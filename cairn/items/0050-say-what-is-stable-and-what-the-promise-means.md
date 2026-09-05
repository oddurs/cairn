---
id: 50
title: Say what is stable, and what the promise means
type: docs
status: backlog
milestone: v1.0
created: 2026-09-05
updated: 2026-09-05
priority: p0
effort: m
---

## Problem

The item format has a version, a written policy and a corpus that pins it. It
is the only part of cairn that has made anybody a promise.

Everything else people will build on is undefined. `--json` shapes, `--plain`
columns, exit codes, the filter grammar, the MCP tool schemas, the interchange
document — all of them are things somebody will script against, and none of
them says whether it may change on a Tuesday. A tool cannot be the obvious
choice for work people depend on while its output is an accident of
implementation.

## Proposal

A compatibility chapter covering four surfaces, each with the same rules the
file format already has:

| Surface | Promise |
| --- | --- |
| Item format | Versioned; additive in minor releases; migration otherwise |
| CLI output for programs — `--json`, `--plain`, `--ids`, exit codes | Fields may be added; never removed or renamed in a minor release |
| Filter grammar and field names | Additive only |
| MCP tool names and schemas | Additive only; a removed tool is a major release |
| Interchange document | Its own version, already present |

And the things explicitly **not** promised, which matters as much: the exact
wording of human-facing output, colour choices, table column widths, the order
of anything not documented as ordered. Those are how the tool stays improvable.

Exit codes get written down too: 0 success, 1 the operation failed or found
problems, 2 the command line was wrong.

## Acceptance criteria

- [ ] Every scriptable surface is named, with its promise
- [ ] What is deliberately unstable is named too
- [ ] Exit codes documented and consistent across commands
- [ ] A test asserts the documented JSON fields exist, so removing one fails

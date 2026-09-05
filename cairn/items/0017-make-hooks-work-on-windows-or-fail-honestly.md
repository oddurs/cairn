---
id: 17
title: Make hooks work on Windows, or fail honestly
type: bug
status: done
milestone: v0.1
assignee: claude
created: 2026-09-04
updated: 2026-09-04
priority: p1
sprint: s1
effort: m
---

## Problem

`hooks.rs` branches to `cmd /C` on Windows. That code path has never executed.
`cmd` quoting differs enough from `sh` that a hook written on Linux will
probably misbehave, silently, in a feature whose whole point is running the
user's own code.

## Proposal

Execute it under test on Windows. Where the shells genuinely differ, document
the difference rather than papering over it; prefer PowerShell if `cmd`'s
quoting proves untenable. A hook that cannot run must say so, not half-run.

## Acceptance criteria

- [ ] Hook tests run on Windows CI
- [ ] Quoting behaviour documented per platform
- [ ] An unrunnable hook produces a clear diagnostic, never partial execution

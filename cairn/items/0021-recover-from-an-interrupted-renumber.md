---
id: 21
title: Recover from an interrupted renumber
type: bug
status: done
milestone: v0.2
assignee: claude
created: 2026-09-04
updated: 2026-09-04
priority: p1
sprint: s2
effort: s
---

## Problem

`renumber` stages files as `*.md.renumber` before writing them back. A crash
between the two phases leaves staged files that no command knows about, and the
items they hold are invisible.

## Proposal

Detect leftovers on startup of any command that reads items, and either recover
them automatically or report exactly what to do. Never leave a user to work it
out from the filenames.

## Acceptance criteria

- [ ] Interrupting renumber at any phase leaves a recoverable state
- [ ] The next command explains what happened
- [ ] Test kills the process between phases

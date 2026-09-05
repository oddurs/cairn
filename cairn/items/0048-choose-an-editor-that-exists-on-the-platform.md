---
id: 48
title: Choose an editor that exists on the platform
type: bug
status: backlog
milestone: v0.2
created: 2026-09-05
updated: 2026-09-05
priority: p1
effort: s
---

## Problem

`cairn edit` falls back to `vi` when neither `VISUAL` nor `EDITOR` is set. On
Windows there is no `vi`, so the command fails with a message about a missing
program rather than doing anything useful — on a tier 2 platform, in a command
somebody reaches for early.

## Proposal

Fall back per platform: `notepad` on Windows, `vi` elsewhere. If even that is
missing, say which variable to set rather than reporting a spawn failure.

## Acceptance criteria

- [ ] A Windows machine with no EDITOR opens something
- [ ] A missing editor explains how to choose one
- [ ] Covered by a test on every platform

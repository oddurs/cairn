---
id: 23
title: Lock the repository for mutating commands
type: feature
status: done
milestone: v0.2
assignee: claude
created: 2026-09-04
updated: 2026-09-04
priority: p0
sprint: s3
effort: m
---

## Problem

Two concurrent `cairn new` calls both compute `max(id) + 1` and produce the same
id. `cairn renumber` repairs that after the fact, which was an acceptable answer
when the writer was a person. It is not, now that two agents working one backlog
is the advertised use case.

## Proposal

An advisory lock file held for the duration of mutating commands, covering id
allocation and the write itself. Must work on all three platforms; must not
deadlock if a process dies holding it — stale locks time out and say so.

Read commands never take the lock: listing must not block behind a write.

## Acceptance criteria

- [ ] Id allocation happens under the lock
- [ ] A stale lock is detected and broken with a clear message
- [ ] Read commands are never blocked
- [ ] Works on Linux, macOS and Windows

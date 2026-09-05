---
id: 19
title: Write item files atomically
type: bug
status: done
milestone: v0.1
assignee: claude
created: 2026-09-04
updated: 2026-09-04
priority: p0
sprint: s2
effort: m
---

## Problem

`Item::save` calls `fs::write`, which truncates before writing. A crash, a full
disk, or a killed process between those two moments leaves a truncated item —
and the repository is the database, so that is data loss.

## Proposal

Write to a temporary file in the same directory, fsync it, then rename over the
target. Rename is atomic within a filesystem on every supported platform.
fsync the directory on POSIX so the rename itself is durable.

## Acceptance criteria

- [ ] No code path truncates a file in place
- [ ] Interrupting a write leaves either the old file or the new one, never
      a partial one
- [ ] A kill-during-write loop leaves the corpus valid every time
- [ ] Verified on all three platforms

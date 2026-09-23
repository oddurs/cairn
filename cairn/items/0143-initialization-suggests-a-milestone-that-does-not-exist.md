---
id: 143
title: Initialization suggests a milestone that does not exist
type: bug
status: done
milestone: v0.3
assignee: codex
created_by: codex
created: 2026-09-23
updated: 2026-09-23
closed_at: 2026-09-23
priority: p2
effort: s
area: cli
---

## What happens

The README smoke test in 0134 found that init --preset minimal --bare prints a next command using --milestone v0.1, although --bare creates no milestone. Following that suggestion fails.

## What should happen

Initialization should suggest a command that succeeds with the project it just created.

## Acceptance criteria

- [x] Bare initialization suggests creation without a nonexistent milestone.
- [x] Standard initialization with example milestones still offers the first milestone.
- [x] The printed commands are exercised against the real binary and the project checks pass.

## 2026-09-23

Verified the printed next command in fresh standard --bare, minimal --bare, and standard-with-examples projects: each command succeeds and strict validation passes. The broader minimal-with-examples probe exposed an existing keyless-container fixture bug, now recorded separately as 0144; the hint change does not cause or repair it.

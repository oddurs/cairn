---
id: 164
uid: 95afd0a8-c142-48df-8f27-72478799a4a0
title: Advance the Harrow pin to its items-as-prompts release
type: chore
status: done
milestone: v1.0
assignee: oddurs
created_by: cairn-26
created: 2026-09-28
updated: 2026-09-28
closed_at: 2026-09-28
priority: p1
effort: s
area: integration
---

## Why

Harrow v0.8 is merged (harrow#110–#121). It reads a Result as cairn does, and its
agreement suite now has a gate for it: every item's Result must read the same
in both tools, null for null. spec/harrow-revision still pins 0ae9c09, which
predates that gate, so cairn's own CI doesn't run it.

## Acceptance criteria

- [x] spec/harrow-revision pins Harrow d3ed79c
- [x] make agreement passes against it, including every_item_concludes_the_same_in_both_tools

## 2026-09-28

make agreement HARROW_REPO=<detached worktree of harrow at d3ed79c> with cairn main 9c29249: 6 passed — the five that ran before, and every_item_concludes_the_same_in_both_tools, harrow's gate that each item's Result reads the same in both tools (it fails in harrow when its fence rule is removed).

## Result

Cairn's CI builds Harrow d3ed79c, its items-as-prompts release, and holds cairn to its agreement suite, including the gate that every item's Result reads the same in both tools.

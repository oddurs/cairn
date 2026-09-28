---
id: 153
uid: 835b4f34-ebe7-44b8-b8f7-3055866aaad0
key: prompts
title: Items as prompts
type: milestone
status: done
depends_on:
- 135
created: 2026-09-27
updated: 2026-09-27
closed_at: 2026-09-27
priority: p2
---

An item reads as a prompt that outlives the session, and cairn treats it as
one: it compiles an item and everything it rests on into what an agent should
read, carries a finished item's result to the work that depends on it, splits
a prompt too large to run into sub-prompts, and says when a prompt will be
misread.

## Release gate

Every command in this milestone works on this repository's own backlog, and an
agent starting an item can begin from `cairn prompt <ID>` alone. No runner, no
service: each piece is a view, a convention or an ordinary write over the
record that already exists (0142).

## Sequence

The decision first, then Results, because the compiler quotes them; then the
compiler; then split and the prompt checks, which the compiler also surfaces.

## 2026-09-27

Verified on main at 2fb5f31, after #112, #113, #115 and #116: make check 696 passed, 0 failed; check --render --strict and check --prompts clean on 161 items. Spot-checked there: prompt 0147 hands over 0145's recorded Result; prompt --json carries layers and checks; split 0147 --dry-run proposes its three steps in order; show --json carries result; AGENTS.md starts the loop from cairn prompt. Follow-ups outside the milestone: 0159, 0160, 0161 (#114).

## Result

Items are prompts, and cairn treats them as such without running anything: a ## Result is what dependents quote (close --result, spec §10.2); cairn prompt / MCP prompt_item compiles an item and what it rests on into seven layers; cairn split turns numbered steps into ordered sub-items; prompt checks advise in cairn prompt and check --prompts. Shipped in #112, #113, #115, #116.

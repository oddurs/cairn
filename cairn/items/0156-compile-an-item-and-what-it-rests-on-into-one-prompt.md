---
id: 156
uid: 5c837fc8-6894-400f-96a6-5fede12211ec
title: Compile an item and what it rests on into one prompt
type: feature
status: planned
milestone: prompts
depends_on:
- 155
created: 2026-09-27
updated: 2026-09-27
priority: p1
area: cli
effort: l
---

## Problem

An agent starting an item reads AGENTS.md, runs `cairn show`, then follows
each dependency and the milestone by hand to learn what the work rests on.
Nothing assembles it, so each agent assembles it differently, or not at all.

## Approach

`cairn prompt <ID>` prints the item as the prompt an agent should read, in
this order, each layer labelled with where it came from:

1. **How this project works**: the loop, with this item's id filled in (claim,
   note, tick, close), from the same source as the generated instructions.
2. **The outcome**: each grouping container the item is filed under (its
   milestone) and each ancestor through a rollup field, as title and first
   paragraph.
3. **What this builds on**: each direct dependency: title and status, then its
   Result if finished (0155), else its last dated note, else nothing; an
   unfinished one is named as a blocker.
4. **The task**: the title and the body, less criteria, Result and dated notes.
5. **Done when**: the criteria, open ones numbered as `cairn tick` numbers them.
6. **What earlier runs learned**: the item's dated notes, oldest first.
7. **When you stop**: note, tick, close with a result, or release with a reason.

Read-only, works at any format cairn reads. `--json` returns the layers as
data. MCP gains `prompt_item`. The generated agent instructions tell an agent
to start from `cairn prompt <ID>`.

## Acceptance criteria

- [ ] `cairn prompt <ID>` prints the seven layers in order, each naming its source
- [ ] A finished dependency contributes its Result, or its last note when it has none; an unfinished one is named as a blocker
- [ ] Open criteria carry the numbers `cairn tick` accepts
- [ ] `--json` and MCP `prompt_item` return the same layers
- [ ] The generated agent instructions start the loop from `cairn prompt`, and AGENTS.md is regenerated
- [ ] Run against this repository's own items, the output is what an agent needs and nothing it does not
- [ ] `make check` passes

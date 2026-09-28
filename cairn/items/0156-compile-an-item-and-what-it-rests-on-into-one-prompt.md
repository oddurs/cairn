---
id: 156
uid: 5c837fc8-6894-400f-96a6-5fede12211ec
title: Compile an item and what it rests on into one prompt
type: feature
status: done
milestone: prompts
assignee: oddurs
depends_on:
- 155
created: 2026-09-27
updated: 2026-09-27
closed_at: 2026-09-27
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

- [x] `cairn prompt <ID>` prints the seven layers in order, each naming its source
- [x] A finished dependency contributes its Result, or its last note when it has none; an unfinished one is named as a blocker
- [x] Open criteria carry the numbers `cairn tick` accepts
- [x] `--json` and MCP `prompt_item` return the same layers
- [x] The generated agent instructions start the loop from `cairn prompt`, and AGENTS.md is regenerated
- [x] Run against this repository's own items, the output is what an agent needs and nothing it does not
- [x] `make check` passes

## 2026-09-27

Checked against this repository's own items: prompt 0148 and prompt 0156. 0156's dependency layer is 0155's recorded Result, which is all 0156 needs of it; 0148's shows the fallback, 0145's last note, because 0145 finished before Results existed. Refinements from reading the output: the item's own headings and note dates sit a level below the layer headings; a dependency's last note is introduced with its date rather than its raw heading; the task starts at the body because the title heads the whole prompt (a small departure from the Approach, which said title and body).

## 2026-09-27

Review (/code-review medium) found four real defects, each fixed with a test that fails without it: with a criteria_section configured, checkboxes outside it vanished from every layer (now exactly the lines Done when shows are removed from the task, by line number); prompt refused keys that show accepts (now resolves keys first); a body under a top-level # heading swallowed its notes into the task (note headings split at any level); prose in the criteria section was dropped (kept when anything besides boxes remains).

## Result

cairn prompt <ID> (and MCP prompt_item) compiles an item into seven layers, each naming its source: how the project works, the outcome (milestone and rollup parents, outermost first, first paragraph), what each dependency concluded (Result, else last note, else a blocker), the task (body less criteria lines, Result and notes), done-when (open criteria numbered for tick), notes oldest first, and what to leave behind. --json returns the layers. Read-only; the generated instructions start the loop from it.

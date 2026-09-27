---
id: 158
uid: 8eed6479-1efa-4662-aa1f-1dfb1b4b28bb
title: Say when a prompt will be misread
type: feature
status: planned
milestone: prompts
depends_on:
- 156
created: 2026-09-27
updated: 2026-09-27
priority: p2
area: cli
effort: m
---

## Problem

Some items will be misread by whatever runs them: no acceptance criteria, so
no definition of done; an empty context; a dependency that finished without
saying what it concluded; criteria ticked with no note saying how they were
proven. Nothing says so until an agent gets it wrong.

## Approach

Prompt checks over open items, as advice:

- no acceptance criteria: nothing defines done;
- no context: the body has no text beyond headings, criteria and template;
- a finished dependency with no Result (0155): this prompt gets a note instead;
- ticked criteria and no dated note: done, with no record of how it was proven.

`cairn prompt <ID>` prints the ones that apply to that item after the
prompt. `cairn check --prompts` reports them for every open item as warnings,
so `--strict` fails on them only when asked. Plain `cairn check` is
unchanged, so existing CI is unaffected.

## Acceptance criteria

- [ ] Each of the four checks has a case that fires and one that does not
- [ ] `cairn prompt` shows the checks that apply to its item
- [ ] `check --prompts` reports them for open items; plain `check` does not
- [ ] This repository's own open items pass `check --prompts`, or the findings are fixed in the items
- [ ] `make check` passes

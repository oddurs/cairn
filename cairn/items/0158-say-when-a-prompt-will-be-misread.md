---
id: 158
uid: 8eed6479-1efa-4662-aa1f-1dfb1b4b28bb
title: Say when a prompt will be misread
type: feature
status: done
milestone: prompts
assignee: oddurs
depends_on:
- 156
created: 2026-09-27
updated: 2026-09-27
closed_at: 2026-09-27
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

- [x] Each of the four checks has a case that fires and one that does not
- [x] `cairn prompt` shows the checks that apply to its item
- [x] `check --prompts` reports them for open items; plain `check` does not
- [x] This repository's own open items pass `check --prompts`, or the findings are fixed in the items
- [x] `make check` passes

## 2026-09-27

Run against this backlog, check --prompts found four real problems, fixed in the items: 0001 had no criteria and no context outside its notes (now a Problem and two criteria, one ticked on its 2026-09-05 note's evidence); 0145 and 0154 finished without a Result, so 0147 and 0157 were handed a note or nothing (both now record one; closed_at unchanged). After that, check --prompts reports nothing.

## 2026-09-27

Review (/code-review medium) found three real defects, each fixed with a test that fails without it: a bug filed from its template was never 'no context', because template scaffolding like its bare '1.' counted (lines from the type's template no longer count); a Released-by note silenced 'ticked with no note' (only dated notes count as evidence); a dropped dependency was called finished, with advice to close it as done (dropped dependencies get their own advice).

## Result

Prompt checks, as advice: no acceptance criteria; no context beyond headings, criteria and the type's template; a dependency done without a Result, or dropped; ticked criteria with no dated note. cairn prompt ends with those that apply; cairn check --prompts reports them for open, non-container items as warnings, so --strict fails only when asked; plain check is unchanged.

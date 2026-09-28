---
id: 155
uid: 664c2b17-8fd4-4c00-a11e-517e8a6026d9
title: A finished item records a Result that dependents can quote
type: feature
status: done
milestone: prompts
assignee: oddurs
depends_on:
- 154
created: 2026-09-27
updated: 2026-09-27
closed_at: 2026-09-27
priority: p1
area: format
effort: m
---

## Problem

When 0145 closed, what it concluded lived in four dated notes and a PR. 0148
depended on it and had to reread all of it to learn the one paragraph that
mattered. A finished item has no place for its answer, so nothing downstream
can quote it.

## Approach

- A `## Result` section, found at any heading level and without regard to case,
  is the item's answer. It is ordinary body text: no new key, no format change.
  The specification gains it as a non-normative convention beside acceptance
  criteria (§10).
- `cairn close <ID> --result "<TEXT>"` writes it, replacing an existing Result
  section rather than adding a second. MCP `close_item` takes `result`.
- JSON (`show`, `list`, `export`, MCP) carries `result`, the section's text,
  when there is one, as it carries `uid`.
- Closing an item that open work depends on, with no Result, prints a hint
  naming the dependents that will have nothing to quote. Never an error.

## Acceptance criteria

- [x] `close --result` writes one `## Result` section, and a second close replaces it
- [x] `show --json` and MCP carry `result` when there is one, and omit it otherwise
- [x] MCP `close_item` accepts `result`
- [x] Closing with open dependents and no result hints at them; with a result, or with none waiting, it says nothing
- [x] The specification and the manual describe the convention
- [x] `make check` passes

## 2026-09-27

JSON carries result only when there is one, as it carries uid. Emitting null for every item changed what every existing reading of an item returns, and the golden corpus (exact comparison of show --json) caught it; editing those expectations is reserved for format changes.

## 2026-09-27

Review (/code-review medium) found five real defects, each now fixed with a test that fails without the fix: replacing a result could erase notes after a hand-written # Result (a Result section now also ends at a note's heading); a result carrying its own ## heading was cut short (written headings are pushed below the Result heading); an unclosed fence hid the Result (a fence nobody closed is not a fence); checkboxes in a result counted as criteria (a Result section contributes none, §10.1 amended); the hint told dropped items to re-close as done (only done items get it). Golden corpus gained three Result cases, and spec/reader.py implements §10.2, so conformance agrees on 66 cases.

## Result

A finished item's ## Result section (spec §10.2) is what it concluded: close --result writes it, replacing an earlier one; JSON carries result when present; MCP close_item takes it. A Result ends at the next heading at its level or above, or at a note's heading; fences are skipped, unclosed ones are not fences; it never contributes criteria. Closing done work that others depend on without a result names them.

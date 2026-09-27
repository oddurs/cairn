---
id: 155
uid: 664c2b17-8fd4-4c00-a11e-517e8a6026d9
title: A finished item records a Result that dependents can quote
type: feature
status: planned
milestone: prompts
depends_on:
- 154
created: 2026-09-27
updated: 2026-09-27
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
- JSON (`show`, `list`, `export`, MCP) carries `result`: the section's text, or
  null.
- Closing an item that open work depends on, with no Result, prints a hint
  naming the dependents that will have nothing to quote. Never an error.

## Acceptance criteria

- [ ] `close --result` writes one `## Result` section, and a second close replaces it
- [ ] `show --json` and MCP carry `result`; an item without one has null
- [ ] MCP `close_item` accepts `result`
- [ ] Closing with open dependents and no result hints at them; with a result, or with none waiting, it says nothing
- [ ] The specification and the manual describe the convention
- [ ] `make check` passes

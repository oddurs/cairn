---
id: 154
uid: 90515abb-b0b0-41e6-bd6f-dc87c26aa1f0
title: An item is a prompt, and cairn compiles it rather than runs it
type: decision
status: done
milestone: prompts
created: 2026-09-27
updated: 2026-09-27
closed_at: 2026-09-27
priority: p1
area: direction
---

## Context

Working this repository with agents showed the shape of an item: context
(Problem), an instruction (Approach), a definition of done (acceptance
criteria), and memory (dated notes written for the next run). An agent starting
0148 needed more than 0148: what 0145 concluded, why 0146 was decided, the
outcome v1.0 serves, and the loop in AGENTS.md. It assembled that by hand, and
0147's numbered steps became separate work without ever being separate items.
The concept page: https://claude.ai/artifact/Q6VyChwcqRPHAR49a5zxFv

## Decision

An item is a prompt, and cairn compiles it rather than runs it.

- `cairn prompt <ID>` is a read-only view, like `render`: the item and
  everything it rests on, in the order a model should read it. The file stays
  the source.
- A finished item's `## Result` section is what dependents quote. It is a body
  convention, not a new key, so no format change.
- `cairn split <ID>` turns a numbered Approach into ordinary child items.
- Prompt checks are advice, opt-in with `cairn check --prompts` and shown by
  `cairn prompt`, so existing backlogs and `check --strict` in CI are unchanged.

## Boundary

Within 0142: no agent runner, no service, no second source of truth, no new
configuration key. Everything is derived from items, the schema and the
generated instructions.

## Acceptance criteria

- [x] Owner-approved: "implement it as a cairn milestone"
- [x] Each proposal is a view, a convention or an ordinary write over existing data

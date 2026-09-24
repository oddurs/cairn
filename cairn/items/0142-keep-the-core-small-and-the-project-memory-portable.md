---
id: 96ea5ce3-b87d-4698-9fad-c8dcdd289b90
title: Keep the core small and the project memory portable
type: decision
status: done
milestone: v0.3
assignee: codex
created_by: codex
created: 2026-09-23
updated: 2026-09-23
closed_at: 2026-09-23
priority: p1
effort: s
area: direction
---

## Context

After several weeks of daily use, the objective is a tool worth keeping for a lifetime: lightweight, Git-native, agent-first, and flexible. 0134 assesses the implementation and records the evidence.

## Options and tradeoffs

A larger all-in-one tracker would add interface, hosting, orchestration, and integration maintenance. A bare list of tasks would lose the reasoning that makes the existing items valuable. The current split already supports a better path: a small durable record with multiple ways to work it.

## Decision

Cairn is project memory versioned with the code: intent, work, decisions, and evidence in ordinary Markdown under a project-owned schema. Git carries the history and review. Cairn validates and changes the record. Harrow watches it and lets a person steer it. Agents consume the same record through CLI/JSON or MCP.

The next investment is the shared daily loop, branch/worktree semantics, and agreement between readers. Before 1.0, establish compatibility and recovery evidence. Do not add a service, embedded interpreter, agent runner, shared Rust library, or another source of truth to solve problems the existing boundaries already address.

Keep sequential integer IDs while 0067 is evaluated with evidence from 0138. Do not make a permanent identifier promise or change the format as a side effect of this assessment.

## Revisit when

A recorded task cannot be completed with the format, schema, queries, hooks, or an external consumer; or real multi-project use demonstrates a boundary that harms the record. Record the evidence before changing the boundary.

## Acceptance criteria

- [x] The README explains the product and the Cairn/Harrow/Git responsibilities.
- [x] The repository configuration makes the selected workflow usable.
- [x] The assessment links this decision to an executable next slice.

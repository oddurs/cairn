---
id: 139
title: Recover the reason for a change after a long absence
type: feature
status: backlog
milestone: v1.0
owner: oddurs
created_by: codex
created: 2026-09-23
updated: 2026-09-23
priority: p1
effort: m
area: workflow
---

## Problem

The record is intended to outlive a session, a model, and its original author. Today search hides finished items by default, while most of this project's useful reasoning is finished. A new agent following only the generated loop can therefore miss why an appealing feature was refused.

## Proposal

Evaluate a return-to-project workflow using search --all, saved decision/history views, show, log, and scoped JSON. Start with recipes and observation. Add a bounded context output only if those existing surfaces demonstrably fail; use an existing command when possible.

Use actual questions: Why no library? What did the last branch change? Why keep integer IDs? Which decision would this proposal reverse? What is waiting for a person?

## Acceptance criteria

- [ ] A human and a fresh agent answer the recorded questions using a backlog with at least 1000 mostly closed items.
- [ ] Retrieval includes relevant dropped and superseded reasoning without dumping every item's body.
- [ ] Any new output has a bounded size, source item references, and a clear distinction between recorded fact and inference.
- [ ] The experiment and the decision to build or not build anything are recorded.

## Boundaries

No embeddings service, vector database, automatic prose summariser, or parallel memory store. The source remains the items and Git.

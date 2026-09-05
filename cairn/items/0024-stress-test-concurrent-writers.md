---
id: 24
title: Stress-test concurrent writers
type: chore
status: done
milestone: v0.2
assignee: claude
created: 2026-09-04
updated: 2026-09-04
priority: p0
sprint: s3
effort: s
---

## Problem

The concurrency story is untested. Nothing proves two agents cannot corrupt a
backlog.

## Proposal

A test that runs many `cairn new`, `cairn claim` and `cairn set` calls in
parallel against one project and asserts the invariants: no duplicate ids, no
lost writes, no unparseable files, `cairn check` clean afterwards.

## Acceptance criteria

- [ ] 50 concurrent writers, zero duplicate ids
- [ ] Every write either lands or fails loudly; none silently lost
- [ ] Runs in CI on all three platforms

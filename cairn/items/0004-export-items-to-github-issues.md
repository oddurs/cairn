---
id: 4
title: Export items to GitHub issues
type: feature
status: dropped
milestone: v0.2
depends_on:
- 10
created: 2026-09-04
updated: 2026-09-05
priority: p2
effort: l
---

## Problem

## Proposal

## Acceptance criteria

- [ ]

## Dropped, 2026-09-05

Superseded by a decision about shape rather than scope.

Two-way sync between two systems of record is a tar pit: identifier mapping,
conflict resolution, update loops, rate limits, partial failure. It is an
entire product, and building it would eat this one.

The repository is the only system of record. GitHub issues are an inbox, and
`ROADMAP.md` is the public face — one direction each, no reconciliation. What
remains useful from this item is small and is filed separately: closing an
imported issue with a pointer to the item it became.

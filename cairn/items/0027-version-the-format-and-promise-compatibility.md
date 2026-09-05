---
id: 27
title: Version the format and promise compatibility
type: feature
status: done
milestone: v1.0
assignee: claude
created: 2026-09-04
updated: 2026-09-04
priority: p0
sprint: s4
effort: m
---

## Problem

Durability is a promise, and cairn has not made one. Nothing says which changes
to the format are allowed in a patch release.

## Proposal

A `format` version in `cairn.toml`, and a written policy:

- Patch and minor releases may add optional keys and nothing else.
- Removing or repurposing a key requires a major version and a migration.
- A reader must preserve keys it does not understand, so an older cairn cannot
  silently strip data written by a newer one.

Plus `cairn migrate`, which is a no-op today and exists so the path is real
before it is needed.

## Acceptance criteria

- [ ] `format` recorded in cairn.toml and checked on load
- [ ] A newer format produces a clear message, not a misparse
- [ ] Policy published in the manual
- [ ] `cairn migrate` exists and is exercised by a test

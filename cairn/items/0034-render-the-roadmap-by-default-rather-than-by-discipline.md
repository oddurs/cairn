---
id: 34
title: Render the roadmap by default rather than by discipline
type: bug
status: done
milestone: v0.1
created: 2026-09-05
updated: 2026-09-05
priority: p1
effort: s
---

## Problem

Also from dogfooding. A project one day old already had a stale `ROADMAP.md`,
with the staleness committed: an item had moved to `doing` and the rendered
roadmap still showed it under backlog.

The hooks that would have prevented it shipped commented out, so keeping the
roadmap current was left to whoever remembered. That is precisely the rot cairn
exists to prevent, and the default configuration permitted it.

## Fix

`cairn init` now enables the render hooks. Creating, changing or removing an
item re-renders the roadmap. A project that would rather render by hand comments
them out, or passes `--no-hooks`.

## Acceptance criteria

- [x] A fresh project's roadmap is current after any item operation
- [x] `--no-hooks` still gives a project full manual control
- [x] The drift detection in `render --check` is still tested, by causing drift
      deliberately

---
id: 33
title: Order milestones the way their author declared them
type: bug
status: done
milestone: v0.1
created: 2026-09-05
updated: 2026-09-05
priority: p1
effort: s
---

## Problem

Found by dogfooding in another project. Its `cairn.toml` declares:

    [[milestone]] name = "m0-proof"                        # no due date
    [[milestone]] name = "m1-device"   due = 2026-11-01
    [[milestone]] name = "m2-firmware" due = 2027-01-15
    [[milestone]] name = "m3-craft"    due = 2027-04-01
    [[milestone]] name = "later"                           # no due date

`m0-proof` is declared first and named to sort first. cairn put it fourth,
because the rule was "dated milestones by date, undated last". That rule
overrode an ordering the author had already expressed unambiguously — and it
did so in the rendered roadmap, which is the artefact other people read.

## Fix

An undated milestone keeps the position it was declared in, by taking the date
of the next dated milestone after it. A trailing `later` with nothing dated
after it still sorts last, which is what that name means.

## Acceptance criteria

- [x] `m0-proof` declared first comes first, in `milestone list` and in ROADMAP.md
- [x] `later` declared last still comes last
- [x] Dated milestones still order by date regardless of declaration order

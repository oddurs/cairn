---
id: 55
title: Recordings embedded the day they were made
type: bug
status: done
milestone: v0.2
created: 2026-09-06
updated: 2026-09-06
priority: p1
effort: s
---

## Problem

Caught by continuous integration on an unrelated pull request. The recorded
demo and the website's samples both create items, and an item records the date
it was created. So a recording made today differs from the one committed
yesterday, and the job that guards them fails every night at midnight
regardless of whether anything changed.

That is worse than a nuisance. A check which fails for reasons unconnected to
what it guards is one people learn to ignore, and then it stops guarding
anything.

## Fix

Honour `SOURCE_DATE_EPOCH`, the reproducible-builds convention: a Unix
timestamp standing in for "now". Both recorders pin it, so a recording is a
function of the program rather than of the day it was made.

Worth having beyond the recordings — anybody packaging cairn for a distribution
that checks reproducibility gets it for free — which is why it is that variable
rather than something invented for the purpose.

## Acceptance criteria

- [x] Two runs on different days produce identical recordings
- [x] A malformed value is ignored rather than fatal
- [x] Documented in the manual, since honouring a convention is only useful if
      somebody knows

## 2026-09-06

Fixed by honouring SOURCE_DATE_EPOCH. Verified by running both recorders twice and diffing, and by a test that pins the clock and checks the recorded date.

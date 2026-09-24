---
id: 7934b7b6-9351-4c7c-94da-69adecb4e412
title: Exercise years of history and interrupted work
type: chore
status: backlog
milestone: v1.0
owner: oddurs
created_by: codex
created: 2026-09-23
updated: 2026-09-23
priority: p1
effort: l
area: format
---

## Purpose

"Keep it for a lifetime" needs evidence about old data and failure, beyond a growing number of feature tests. Cairn already has atomic writes, concurrent-process tests, frozen corpora, fuzzing, and an independent reader. Extend those foundations where the cost of failure is lost intent.

## Approach

Exercise mixed-format history and 1000/10000-item backlogs, with mostly finished items, renamed keys, and interrupted writes/renumbers. Record read, search, render, and Harrow startup timings on named hardware. Distinguish reading costs, hook/render costs, and durable writes.

Review two specific code paths found during 0134: Lock::acquire_unchecked breaks locks by age alone after 300 seconds, and Store::load_lenient calls recover_staged during reads. Establish whether a still-running slow writer or an interrupted rename can race another process before changing either path. These are risks from inspection, not claimed reproduced data-loss bugs.

## Acceptance criteria

- [ ] A reproducible long-history fixture exercises old formats, non-ASCII text, mostly closed work, and malformed neighbour files.
- [ ] Measurements state hardware, tool versions, item count, hooks, and warm/cold conditions; regressions have explicit review thresholds.
- [ ] Crash/restart and long-running-writer tests establish lock ownership and recovery behaviour without losing an acknowledged item.
- [ ] An independent reader still recovers the durable record without Cairn or Harrow.
- [ ] Any performance change retains a stated durability contract and passes make durability.

## Boundaries

Keep per-file durability unless new measurements justify a separately reviewed decision. 0127 records why weakening it was declined.

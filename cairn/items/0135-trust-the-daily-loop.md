---
id: f13ddc5e-9c3d-4b36-bd2e-52476b97db8b
key: v0.3
title: Trust the daily loop
type: milestone
status: done
assignee: codex
owner: oddurs
created_by: codex
depends_on:
- 6088a6f0-71c4-465a-8a55-bc1053949b09
created: 2026-09-23
updated: 2026-09-23
closed_at: 2026-09-23
priority: p2
---

People and agents can choose approved work, carry it through a branch, and review the result in Cairn or Harrow without disagreeing about project state.

## Release gate

The approved queue, Cairn–Harrow agreement, and branch/worktree workflow have executable checks and a recorded end-to-end use session. The fixes already on main have a verified release path. The author's daily use supplies immediate feedback; outside research in 0066 runs alongside implementation and informs the 1.0 promise.

## Sequence

0136, 0137, and 0138 are the next engineering slice. 0141 packages the fixes already present as 0.2.2 without waiting for the full v0.3 outcome. 0066 is a human task, not work an unattended coding agent can finish.

No due date is assigned. The previous dates did not describe the released software or a real deadline.

## 2026-09-23

Recorded the release end-to-end session in a clean Git project: initialized with Git integration; approved one p1 item alongside a p0 untriaged idea and a p0 external wait; generated view-scoped instructions; next/claim selected only the approved item; Harrow agreed before and after the claim. Committed the assignment, implemented on a branch, recorded reasoning, verified and ticked both criteria, closed it, compared both history views, and appended a completion note without changing closed_at. Reviewed cairn log --range, merged, and verified the clean project with strict render checks and an empty approved queue. The executable branch/worktree, view-policy, and later-edit date tests retain the corresponding regression coverage.

## 2026-09-23

Shipped v0.3.0 from verified commit 8044313 through PR #98 (merged as 015532e). All 14 PR checks and main CI passed; coverage is 92.58% of regions and 94.50% of lines. Final make durability passed 567 regular tests, two soak tests (400 operations with seed 24301; four writers with 25 operations each), 20,000 fuzzed invocations (seed 12648430), and all 36 format cases. Pinned Harrow agreement, documentation, generated recordings and dependency audit also passed.

## 2026-09-23

Release run 35936086512 published all five platform binaries and the source archive at https://github.com/oddurs/cairn/releases/tag/v0.3.0. Downloaded all six archives, verified SHA-256 and GitHub provenance for each, and compared the published source archive byte-for-byte with the local git archive. The downloaded macOS binary passed approved-queue create/claim/close, minimal initialization (with explicit render), strict validation, and all three Harrow agreement tests. Homebrew tap PR #2 is merged; upgrade and formula tests pass. Both local Cairn installations now report 0.3.0, and the installed Harrow is built from pinned e7397a5; its package version remains 0.1.0. Harrow doctor accepts all eight views and agrees on all 144 items.

## 2026-09-23

All nine child items and the recorded end-to-end release gate are complete. The roadmap now points toward long-term recovery, retrieval and compatibility evidence without silently selecting new work. No runtime dependency, service or format migration was introduced. Crates.io publication and GPG signing were explicitly skipped because the required secrets are absent; those existing human prerequisites remain tracked in 0001 and 0052, not reported complete.

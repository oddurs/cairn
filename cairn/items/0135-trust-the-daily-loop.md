---
id: 135
key: v0.3
title: Trust the daily loop
type: milestone
status: doing
assignee: codex
claimed: 2026-09-23
owner: oddurs
created_by: codex
depends_on:
- 82
created: 2026-09-23
updated: 2026-09-23
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

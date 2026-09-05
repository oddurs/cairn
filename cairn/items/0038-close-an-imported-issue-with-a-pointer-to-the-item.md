---
id: 38
title: Close an imported issue with a pointer to the item
type: feature
status: done
milestone: v0.2
assignee: claude
created: 2026-09-05
updated: 2026-09-05
priority: p1
effort: s
---

## Problem

`cairn import --from github` brings issues in and leaves them open upstream, so
the same work now exists in two places and immediately begins to diverge. This
is the small, useful part of what two-way sync was meant to solve.

## Proposal

The repository is the system of record; GitHub issues are an inbox. After a
successful import, close each imported issue with a comment naming the item it
became and linking to it.

    cairn import --from github --repo owner/name --close

One direction, no reconciliation, no loop. A reporter who follows the link
lands on the item, in the repository, where the work actually is.

## Acceptance criteria

- [ ] `--close` comments and closes each imported issue, through `gh`
- [ ] The comment links to the item's file at the current revision
- [ ] Import stays idempotent: a re-run neither re-imports nor re-comments
- [ ] Without `--close`, behaviour is exactly as it is today

## 2026-09-05

Built and verified against a real issue: oddurs/cairn#4 was imported, closed upstream, and commented with a pointer to the item file. Idempotence falls out of provenance — a repeated import creates nothing, so there is nothing left to close.

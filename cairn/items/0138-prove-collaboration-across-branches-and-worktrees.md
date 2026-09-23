---
id: 138
title: Prove collaboration across branches and worktrees
type: feature
status: planned
milestone: v0.3
owner: oddurs
created_by: codex
created: 2026-09-23
updated: 2026-09-23
priority: p1
effort: m
area: git
---

## Problem

Claims are atomic within one item directory. They do not coordinate separate working trees. A disposable Git repository demonstrated that `cairn claim 1 --as first` in the main working tree and `cairn claim 1 --as second` in a linked worktree both succeed.

That is consistent with each branch carrying its own project state. It becomes a product failure when "claim so nobody duplicates the work" is read as distributed exclusion. Identifier collisions and diverging claims are related but different problems.

## Proposal

Document and exercise one supported workflow: agree on an assignment in a shared base or explicit coordinator before splitting work, branch with the item and code, merge, validate, and review repairs. Clearly state that offline clones cannot reserve work globally without coordination.

Test clone setup, linked worktrees, existing core.hooksPath, merge, rebase, and cherry-pick. The current post-merge hook alone is not evidence for all those operations. Decide the identifier question in 0067 from these results.

## Acceptance criteria

- [ ] Tests and documentation distinguish same-directory claim exclusion from branch-local claims and show the supported handoff.
- [ ] Two branches adding items and references retain their intent after the supported merge/repair workflow.
- [ ] Git integration is located correctly in linked worktrees and respects an existing hooks configuration.
- [ ] Rebase and cherry-pick behaviour is documented and tested; unsupported automatic repairs are stated explicitly.
- [ ] A reviewer can see both the code change and its item history, with no claim of global locking.

## Boundaries

Git remains responsible for commits, branches, and transport. No remote claim service, mandatory background process, or automatic commits.

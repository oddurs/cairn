---
id: 1b49f9f9-01fc-49c6-b129-cca33972f28d
title: See work under way in other worktrees before claiming
type: feature
status: done
milestone: v1.0
assignee: Oddur Sigurdsson
created: 2026-09-26
updated: 2026-09-26
closed_at: 2026-09-26
priority: p1
effort: m
area: git
---

## Problem

Agents run one item per worktree, and each `cairn claim` lands in that
worktree's copy of the item. 0138 established that claims exclude writers of
one directory and nothing more. So `cairn claim --next --view next` in a second
worktree takes the item an agent claimed in the first a minute ago, and
`cairn next` on `main` shows nothing under way while four agents work.

Seen in this repository: an item filed and set to `doing` on
`feat/readable-ids-format-5` was invisible from every other checkout.

Harrow's PR #101 annotates its board with the same facts. That helps a person
looking; it does not stop an agent choosing duplicate work, because agents
choose through cairn.

## Proposal

Read the other worktrees of the same repository, derived and read-only, the
way Harrow does, so the two tools agree:

    git worktree list --porcelain
    git -C <other> diff --name-only --merge-base <our HEAD> -- <items>
    git -C <other> ls-files --others --exclude-standard -- <items>

Against the merge-base, so a branch cut before `main` moved does not show old
copies of items it never touched. The diff reads the working tree, so an
uncommitted claim counts. Untracked and added files are items filed there.
Only format 4 and later, and only a worktree on the same format: before UUIDs
an id did not name the same item in two checkouts.

An item is taken elsewhere when that worktree's copy has an assignee or a
status that is no longer open, and the claim is not stale.

- `next`, `claim --next` and MCP `next_items` / `claim_item` skip items taken
  elsewhere and say how many they skipped.
- `claim <ID>` refuses an item taken elsewhere, naming the branch and holder,
  unless `--force`. A stale claim elsewhere is taken over with a note, as a
  stale local one is.
- A claim holds a lock in Git's common directory while it reads and writes, so
  two worktrees claiming at once are serialised and the second sees the first.
- `show` lists each other branch's copy; `cairn worktrees` lists what every
  other worktree has changed or filed.

The record is untouched: filters, `list` and status stay this checkout's.

### Boundaries

Local to one machine. Separate clones still need the committed handoff that
0138 documents. No service, no new stored state (decision 0142).

## Acceptance criteria

- [x] A claim in one linked worktree makes `claim --next` in another pick a
      different item, and `claim <ID>` there refuse without `--force`.
- [x] `next` skips items taken elsewhere and says so; MCP selection agrees.
- [x] An item another worktree never touched is not reported, even when the
      copies differ; a branch cut before `main` moved reports nothing stale.
- [x] Items filed in another worktree, committed or not, appear in
      `cairn worktrees`.
- [x] Two concurrent `claim --next` in separate worktrees never take the same
      item.
- [x] No git, no repository, another format, or a pre-UUID format leave every
      command as it was.
- [x] The collaboration guide and manual describe what is and is not
      coordinated.

## 2026-09-26

Built as proposed. src/worktree.rs surveys other worktrees with one git worktree list, then per worktree in parallel a diff --merge-base (AMR, working tree included) and ls-files --others; about 0.1 s on this repository with four worktrees. Claims take cairn-claim.lock in git's common dir before the item-directory lock. next/claim/MCP share select(), which skips items held elsewhere; claim <ID> refuses naming branch and path. Deliberate difference from Harrow #101: items filed on a branch (added or untracked) are listed by cairn worktrees, because seeing the backlog was the point; they never affect selection, since this checkout does not have them. show --json is untouched (golden interchange shape); cairn worktrees --json is the machine surface. Worktrees on another format are named as unread rather than silently skipped: the two format-5 worktrees in this repository are exactly that case. tests/worktrees.rs has 12 real-git tests; removing the shared lock, the merge-base, the untracked scan, the format guard, the select filter or the stale rule each fails at least one. make check: 605 passed, clippy -D warnings clean, check --render --strict ok.

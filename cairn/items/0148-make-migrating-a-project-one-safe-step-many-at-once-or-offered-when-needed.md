---
id: 148
uid: f47ac607-b1d3-43ee-a52c-ca24fe4f06a7
title: Make migrating a project one safe step, many at once, or offered when needed
type: feature
status: done
milestone: v1.0
assignee: oddurs
depends_on:
- 145
created: 2026-09-26
updated: 2026-09-26
closed_at: 2026-09-26
priority: p1
area: cli
effort: l
---

## Problem

Migrating is a procedure, not a command. From this repository's own format-5
migration: the dry run listed 147 filenames; nothing checked the working tree
was clean or said which unmerged branches carried items; nothing showed the
migration preserved every value, so it took a hand-written script; the commit,
the roadmap re-render and the next steps were all manual. Across about thirty
projects that is thirty repetitions. And the day a newer cairn is installed,
every older project refuses writes until someone remembers the command.

## Approach

- **One safe step.** `cairn migrate` runs a preflight (clean working tree for
  the files it rewrites, `check` where the step needs it, unmerged branches that
  carry items), prints a short dry run in counts, verifies the planned result
  before writing anything — bodies identical, every other value unchanged,
  references mapped consistently — re-renders the roadmap, and says what to do
  next. `--commit` records it as one commit.
- **Many projects.** `cairn migrate --all [DIR]` finds every project under a
  directory, shows each one's format and what migrating it would do, then
  migrates each eligible one, skipping and reporting the rest.
- **Offered on first write.** A write to an older project from an interactive
  terminal asks whether to migrate now, runs the safe step on yes, then carries
  on with the command. Agents, scripts and pipes still get the refusal.

## Acceptance criteria

- [x] A dirty working tree, or a step's failing check, stops the migration before anything is written, and says why
- [x] The dry run summarises in counts and names unmerged branches that carry items; `--verbose` lists files
- [x] A migration verifies its plan before writing, and reports what it verified
- [x] The roadmap is re-rendered, and `--commit` records config, items and roadmap as one commit
- [x] `migrate --all` reports every project under a directory, migrates the eligible, skips the rest with a reason, and exits non-zero if any failed
- [x] An interactive write to an older project offers the migration; a non-interactive one refuses as before
- [x] `make check` passes

## 2026-09-26

Verification runs on the journaled plan, before the journal is written: each item is read back before and after, paired by tag (or file), and must keep its body, line endings and every key except id, uid and id references; every id reference must name the counterpart of what it named; every alias in _legacy-ids.toml must be its item's number. Unit test verification_refuses_anything_but_a_change_of_identity feeds it a changed body, a changed title, a dropped tag and a retargeted reference. Config-only steps (1-2, 2-3) rewrite no item and have nothing to verify.

## 2026-09-26

The offer re-execs the command after migrating, because everything it had read was read at the old format. Checked by hand in a real pty with expect: answering y migrated, verified, printed next steps and then created the item it was asked to; answering n refused as before. The integration test covers the non-interactive refusal; the gate itself is unit-tested (may_offer). migrate --all does not search hidden or build directories, so a checkout's .worktrees copies are not migrated twice.

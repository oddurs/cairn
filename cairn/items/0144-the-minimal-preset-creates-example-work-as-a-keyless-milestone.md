---
id: 144
title: The minimal preset creates example work as a keyless milestone
type: bug
status: done
milestone: v0.3
assignee: codex
owner: oddurs
created_by: codex
created: 2026-09-23
updated: 2026-09-23
closed_at: 2026-09-23
priority: p2
effort: s
area: cli
---

## What happens

During the first-use checks in 0134/0143, a fresh cairn init --preset minimal without --bare produced 0004-adopt-cairn-for-the-roadmap.md with type milestone and no key. cairn check --strict then fails with a warning that nothing can be filed under that keyless milestone.

## Reproduction

1. Start in an empty directory.
2. Run cairn init --preset minimal.
3. Run cairn check --strict.

The minimal schema declares only the milestone type, and write_example chooses a type from the schema. The --bare path documented by the refreshed README does not create this invalid example.

## What should happen

The minimal preset should represent ordinary work simply and pass its own strict validation immediately after initialization. Decide whether ordinary work stays untyped or gains a small task type; do not add more workflow machinery.

## Acceptance criteria

- [x] Minimal initialization with and without example items passes strict validation.
- [x] Its example work is selectable by next and claim, rather than treated as a container.
- [x] Adopting a custom schema does not create work using a grouping type accidentally.

## Scope

This was reproduced in the current release build during assessment. It predates the hint-only fix in 0143 and remains unimplemented; triage it for the first-use workflow.

## 2026-09-23

Included in the v0.3 release check because a lightweight first-use path must not begin invalid or hide its own example from next. The minimal schema now declares one ordinary task type; examples adopted from custom schemas choose a non-grouping type, or stay untyped when none exists. No extra workflow fields or on-disk format change.

## 2026-09-23

Regression tests now pass for minimal initialization with and without examples, next/claim visibility, and adopted grouping-only or grouping-first schemas. Adopted examples also use the actual schedule field and do not invent an undeclared due field. All 54 basics tests pass.

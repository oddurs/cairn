---
id: 144
title: The minimal preset creates example work as a keyless milestone
type: bug
status: backlog
owner: oddurs
created_by: codex
created: 2026-09-23
updated: 2026-09-23
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

- [ ] Minimal initialization with and without example items passes strict validation.
- [ ] Its example work is selectable by next and claim, rather than treated as a container.
- [ ] Adopting a custom schema does not create work using a grouping type accidentally.

## Scope

This was reproduced in the current release build during assessment. It predates the hint-only fix in 0143 and remains unimplemented; triage it for the first-use workflow.

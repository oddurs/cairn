---
id: 29
title: Audit dependencies and licences continuously
type: chore
status: done
milestone: v0.2
assignee: claude
created: 2026-09-04
updated: 2026-09-05
priority: p1
sprint: s5
effort: s
---

## Problem

Thirteen direct dependencies and no check on advisories, licences, or
unexpected sources. For a GPL-3 project, an incompatible transitive licence is
a real risk.

## Proposal

`cargo-deny` in CI covering advisories, licences, bans and sources, plus
Dependabot for updates.

## Acceptance criteria

- [ ] `cargo deny check` runs in CI and is required
- [ ] Licence allowlist reviewed and justified
- [ ] Dependency updates arrive as pull requests

## 2026-09-05

cargo-deny in CI over advisories, licences, bans and sources. The allowlist was written from what is actually in the tree rather than a template, so a new licence is a deliberate decision. cairn's own GPL is a scoped exception rather than a global allow: copyleft for this project, never silently for a dependency. Dependabot groups updates weekly.

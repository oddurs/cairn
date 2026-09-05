---
id: 29
title: Audit dependencies and licences continuously
type: chore
status: backlog
milestone: v0.2
created: 2026-09-04
updated: 2026-09-04
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

---
id: 16
title: Test on Linux, macOS and Windows in CI
type: chore
status: done
milestone: v0.1
assignee: claude
created: 2026-09-04
updated: 2026-09-04
priority: p0
sprint: s1
effort: s
---

## Problem

One CI job, one platform. Cross-platform bugs are found by users.

## Proposal

Matrix the test job over ubuntu-latest, macos-latest and windows-latest, plus a
job pinned to the declared `rust-version` so the MSRV claim is checked rather
than asserted.

## Acceptance criteria

- [ ] Test job runs on all three, required for merge
- [ ] MSRV job builds at the declared minimum
- [ ] A deliberate platform-specific break fails CI

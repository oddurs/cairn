---
id: 14
title: Define and publish platform support tiers
type: docs
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

The README says cairn runs everywhere. Nothing verifies that: CI builds and
tests on Linux only. macOS and Windows are claims, not facts.

## Proposal

Three tiers, stated in the README and the manual, each with an obligation:

- **Tier 1 — Linux** (x86-64, aarch64, musl static). The reference platform.
  Full CI, release binaries, and where behaviour is defined when platforms
  disagree.
- **Tier 2 — macOS and Windows.** Full CI and release binaries. Supported;
  bugs are bugs.
- **Tier 3 — everything else.** Builds from source, no CI, best effort.

A tier is a promise about what CI does, not a sentiment.

## Acceptance criteria

- [ ] Tiers documented in README and manual
- [ ] Each tier's CI obligation is actually configured
- [ ] Release artefacts match the tier table exactly

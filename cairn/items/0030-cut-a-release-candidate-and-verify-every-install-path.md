---
id: 30
title: Cut a release candidate and verify every install path
type: chore
status: doing
milestone: v0.1
assignee: claude
created: 2026-09-04
updated: 2026-09-05
priority: p0
sprint: s5
effort: m
---

## Problem

The release workflow, `install.sh` and the Homebrew formula have never run.
A release pipeline that has not released is a guess.

## Proposal

Tag `v0.1.0-rc.1` and verify each artefact by installing it: the Linux musl
binaries on a clean container, both macOS binaries, the Windows zip, the
`install.sh` path, `brew install`, and `cargo install cairn-md`.

## Acceptance criteria

- [ ] Every Tier 1 and Tier 2 artefact installs and runs `cairn --version`
- [ ] `install.sh` verifies checksums and fails loudly on mismatch
- [ ] Checksums published and verified by hand once
- [ ] Build provenance attested

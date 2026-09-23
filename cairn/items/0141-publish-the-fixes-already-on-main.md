---
id: 141
title: Publish the fixes already on main
type: chore
status: blocked
milestone: v0.3
owner: oddurs
created_by: codex
created: 2026-09-23
updated: 2026-09-23
priority: p1
effort: s
area: distribution
---

## Purpose

The checkout is versioned 0.2.1 but NEWS already describes 0.2.2: successful writes no longer report rename failure, completed work has closed_at, Windows roadmap checks respect line endings, clone setup is diagnosed, and branch changes have log --range.

Ship that accumulated value as a maintenance release before the broader v0.3 work is finished.

## Approach

Follow doc/RELEASING.md from a clean reviewed commit. Align Cargo.toml, Cargo.lock, NEWS, the release tag, artifacts, and the tap. The release is an explicit maintainer action. Neither a crates.io token nor a new GPG policy should silently become a prerequisite for the existing binary/Homebrew path.

## Acceptance criteria

- [ ] The version and release notes agree and make release-check passes for the intended tag.
- [ ] make durability passes for the release candidate.
- [ ] Release archives, checksums, and provenance are verified from a clean install.
- [ ] The documented Homebrew and binary install paths deliver that version.
- [ ] The minimum supported Harrow behaviour is checked and release notes identify any version skew.

## Waiting on

A maintainer choosing and publishing a release. No publication, push, tag, secret change, or external message is part of assessment 0134.

## 2026-09-23

Starting the authorized v0.3 implementation with the required maintenance release. Version 0.2.2 packages the fixes already on main plus the bare-init hint. Harrow 0.1.0 remains readable for format 3; its closed_at filter limitation is explicitly documented in NEWS pending 0137.

## Released by codex

Release candidate 335a4d3 is prepared; waiting for durability/CI and publication verification while implementing the next selected slice.

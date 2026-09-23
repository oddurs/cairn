---
id: 141
title: Publish the fixes already on main
type: chore
status: done
milestone: v0.3
owner: oddurs
created_by: codex
created: 2026-09-23
updated: 2026-09-23
closed_at: 2026-09-23
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

- [x] The version and release notes agree and make release-check passes for the intended tag.
- [x] make durability passes for the release candidate.
- [x] Release archives, checksums, and provenance are verified from a clean install.
- [x] The documented Homebrew and binary install paths deliver that version.
- [x] The minimum supported Harrow behaviour is checked and release notes identify any version skew.

## Waiting on

A maintainer choosing and publishing a release. No publication, push, tag, secret change, or external message is part of assessment 0134.

## 2026-09-23

Starting the authorized v0.3 implementation with the required maintenance release. Version 0.2.2 packages the fixes already on main plus the bare-init hint. Harrow 0.1.0 remains readable for format 3; its closed_at filter limitation is explicitly documented in NEWS pending 0137.

## Released by codex

Release candidate 335a4d3 is prepared; waiting for durability/CI and publication verification while implementing the next selected slice.

## 2026-09-23

Maintenance release v0.2.2 is published from 335a4d3 after full local durability (400 operations, 4 concurrent writers, 20000 fuzz vectors, 36 conformance cases) and green cross-platform CI in PR #97. All six archive checksums pass, the downloaded macOS binary reports 0.2.2, and Homebrew tap PR #1 corrects its stale version and license. A clean brew install and brew test passed. Existing Cargo-installed 0.2.1 shadows Homebrew on this machine; the final v0.3 install will update that active path.

## 2026-09-23

All six archives also passed gh attestation verify against oddurs/cairn. A disposable project created, claimed and closed an item using only the downloaded 0.2.2 binary, then passed check --render --strict. Harrow 0.1.0 baseline agreement passed with its completion-date limitation documented in this release. Release https://github.com/oddurs/cairn/releases/tag/v0.2.2; tap PR https://github.com/oddurs/homebrew-cairn/pull/1. No signing or crates.io availability is claimed.

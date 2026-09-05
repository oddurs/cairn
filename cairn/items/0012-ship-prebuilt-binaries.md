---
id: 12
title: Ship prebuilt binaries
type: chore
status: done
milestone: v0.1
assignee: claude
created: 2026-09-04
updated: 2026-09-05
priority: p0
effort: m
---

## Problem

`cargo install` needs a Rust toolchain. Most of the people who would use a
roadmap tool do not have one, and requiring it filters out most of the audience
before they see anything.

## Proposal

Release binaries for macOS (arm64, x86-64), Linux (x86-64, arm64) and Windows,
attached to a tagged release, plus a Homebrew formula. `cargo-dist` generates
the workflow and the installer script.

## Acceptance criteria

- [ ] `curl … | sh` installs a working binary on macOS and Linux
- [ ] `brew install` works
- [ ] Release artefacts are built in CI from a tag, not by hand

## 2026-09-05

Released. Three attempts: the packaging step copied LICENSE, renamed to COPYING when the repository was laid out the GNU way; then including the spec landed it on top of the project README; then macos-13 sat queued for twenty minutes because GitHub is retiring Intel runners, so x86_64-apple-darwin is now cross-compiled from arm64. All five artefacts publish with checksums.

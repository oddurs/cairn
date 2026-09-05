---
id: 12
title: Ship prebuilt binaries
type: chore
status: doing
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

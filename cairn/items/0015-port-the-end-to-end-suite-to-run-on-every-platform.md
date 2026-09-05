---
id: 15
title: Port the end-to-end suite to run on every platform
type: chore
status: done
milestone: v0.1
assignee: claude
created: 2026-09-04
updated: 2026-09-04
priority: p0
sprint: s1
effort: m
---

## Problem

`tests/cli.sh` holds 120 assertions and is POSIX shell, so it does not run on
Windows at all. A third of the supported platforms is covered by unit tests
only.

## Proposal

Rewrite it as a Rust integration test (`tests/cli.rs`) driving the built binary
through `std::process::Command`. No shell, so it runs identically everywhere,
and `cargo test` becomes the single entry point.

Keep the assertion-per-behaviour granularity — the current suite's value is that
a failure names one behaviour.

## Acceptance criteria

- [ ] Every existing assertion ported, none dropped
- [ ] `cargo test` runs the full suite on Linux, macOS and Windows
- [ ] `tests/cli.sh` removed, not left to rot alongside

---
id: 103
title: Say what make check proves, and add the target that proves the rest
type: chore
status: backlog
milestone: v1.0
depends_on:
- 99
- 100
- 101
- 102
created: 2026-09-07
updated: 2026-09-07
priority: p1
sprint: s10
effort: m
area: build
---

## Problem

`make check` passes, and it proves less than it sounds like.

Both soak tests carry `#[ignore = "long-running"]`, so `cargo test` reports
`0 passed; 2 ignored` and the lock, the atomic write and the identifier
allocator get **no exercise under contention** in the ordinary loop. The fuzz
default is 600 invocations locally against 20,000 in `make fuzz`. Neither
arrangement is wrong — a suite somebody will not wait for is a suite somebody
stops running — but the gap is undocumented, so "check passes" carries more
weight in a contributor's head than it has earned.

There is also no single way to ask for the whole thing. The full battery is
`make check`, then `make soak`, then `make fuzz`, then `make conformance`, then
remembering that the per-format corpora exist. Nobody runs all of it before a
release except by knowing to.

## Proposal

**`make durability`.** One target that runs everything the project has:
the suite, the soak at a serious operation count, the long fuzz, the second
reader over every format, and `cairn check --render --strict` on cairn's own
backlog. It prints the seeds it used, so a failure is reproducible, and it is
the thing to run before tagging.

Not a new command — cairn gains nothing. A Makefile target, which is where the
things cargo does not cover already live.

**A paragraph in `CONTRIBUTING.md`** saying plainly what each level proves:

- `make check` --- the suite, formatting, lints, and cairn's own backlog. What
  every pull request must pass. Does *not* exercise concurrency, and runs a short
  fuzz pass.
- `make durability` --- everything, including contention and the long fuzz. Run
  it before a release, and after touching the lock, the write path, identifier
  allocation, or the merge driver.

**And the same in the manual**, under "Verifying a release", which already exists
and does not currently mention the distinction.

## Why this is the last pass and not the first

Because a `make durability` that runs the suite as it stands would be a longer
way of saying `make check`. The three passes before this one are what give it
something to run.

## Acceptance criteria

- [ ] `make durability` runs the suite, soak, long fuzz, conformance across every format, and the project's own check
- [ ] It prints the seeds it used, so any failure is reproducible
- [ ] `CONTRIBUTING.md` says what `make check` proves and what it does not
- [ ] It names the four things whose change requires `make durability`: the lock, the write path, identifier allocation, the merge driver
- [ ] The manual's release section says the same
- [ ] CI still runs the pieces separately, so a failure names which one

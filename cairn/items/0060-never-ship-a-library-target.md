---
id: 60
title: Never ship a library target
type: chore
status: done
milestone: v0.2
created: 2026-09-06
updated: 2026-09-06
priority: p1
effort: s
---

## Problem

cairn has a `[[bin]]` and no `[lib]`, which is the right shape and happened by
accident rather than decision. Somebody will eventually suggest adding one — it
is the obvious convenience when a second program wants to read items — and the
reason not to is not obvious at all.

A program that links cairn as a library is a derivative work and inherits the
GPL. A program that reads cairn's documented file format is not. That is the
whole difference between a specification other people can build on and a licence
other people must adopt.

So the absence of a library is not an omission. It is the mechanism by which
the format, rather than the code, is the thing to build against — and the
project's strongest asset is the specification, so this is worth defending
deliberately.

## Proposal

Write the reason into `CONTRIBUTING.md`, next to the licensing section where
somebody will look. Then make the rule check itself: a step that fails if
`[lib]` appears in `Cargo.toml`, so the decision cannot be quietly reversed by
somebody who did not know it was a decision.

The alternative for a program that wants cairn's behaviour rather than its
files: run the binary. The output is JSON, the exit codes mean something, and
that interface is on its way to being a contract.

## Acceptance criteria

- [ ] The reason is written down where somebody proposing a library would read it
- [ ] Continuous integration fails on a `[lib]` target
- [ ] The manual points a would-be integrator at the format and the JSON output

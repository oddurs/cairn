---
id: 11
title: Settle the published name
type: chore
status: done
milestone: v0.1
created: 2026-09-04
updated: 2026-09-04
priority: p0
effort: s
---

## Problem

`cargo install cairn` — which the README tells people to run — does not work.
`cairn` is taken on crates.io by an unrelated 0.0.0 crate ("build-gated version
control for Rust projects", 45 downloads, last touched 2026-01-16), and
`cairn-cli` is taken too. The install instructions are currently a promise the
project cannot keep, which is the one thing that must not be true at release.

## Options

1. **Publish as `cairn-md`, keep the binary named `cairn`.** Free today, works
   immediately, costs one line of confusion in the README. Cargo allows the
   crate and binary names to differ.
2. **Ask for the name.** crates.io has a policy for transferring abandoned
   names. Slow and uncertain, but `cairn` is the better name.
3. **Rename outright.** Cheapest now, most expensive later — and the metaphor
   is load-bearing in the documentation.

Recommend 1, and pursue 2 in parallel; if it succeeds, publish `cairn` and keep
`cairn-md` as an alias.

## Acceptance criteria

- [ ] The install command in the README actually works
- [ ] Homebrew formula or tap published under the same name
- [ ] `cairn --version` and the manual agree with the published name

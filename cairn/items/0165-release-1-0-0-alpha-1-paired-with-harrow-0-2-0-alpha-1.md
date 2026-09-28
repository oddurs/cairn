---
id: 165
uid: d5eab41b-a485-4ecc-b60d-d15e08bc412b
title: Release 1.0.0-alpha.1, paired with Harrow 0.2.0-alpha.1
type: chore
status: doing
milestone: v1.0
assignee: oddurs
claimed: 2026-09-28
created_by: cairn-26
created: 2026-09-28
updated: 2026-09-28
priority: p1
effort: s
area: distribution
---

## Why

Main has carried format 5, items as prompts, and the lock and pipe fixes since
v0.3.0, and none of it has been released. Harrow's v0.8 work pairs with exactly
this cairn (its COMPATIBILITY.md names 1.0.0-alpha.1). An alpha, so it's marked
a pre-release and v0.3.0 stays the release installers take.

## Acceptance criteria

- [x] NEWS has a dated 1.0.0-alpha.1 section covering what a user would notice
- [x] make release-check passes for v1.0.0-alpha.1, after the full pre-tag checklist
- [ ] The tag's release workflow publishes binaries, the source tarball and checksums, marked as a pre-release

## 2026-09-28

Pre-tag checklist from doc/RELEASING.md, run on d26adfa: make check 0, make soak 0, make fuzz 0 (20000 invocations, exit statuses 0/1/2), make conformance 0 (66 cases, two readers agree), make agreement 0 against the pinned Harrow d3ed79c, make dist 0 (cairn-1.0.0-alpha.1.tar.gz), make release-check TAG=v1.0.0-alpha.1 0 ('Cargo.toml, NEWS and Cargo.lock all agree') once the dist tarball was moved out of the tree. PR CI 14/14 green. NEWS dated, with a lead paragraph saying it is an alpha paired with Harrow 0.2.0-alpha.1, and sections for writers waiting their turn, taking turns at breaking an abandoned lock, the quiet broken-pipe exit, and MCP id wording. The release workflow marks a tag with a suffix a pre-release and never latest, so v0.3.0 stays what installers take. Harrow's release is harrow-51's (agreed); it will pin this tag.

---
id: 165
uid: d5eab41b-a485-4ecc-b60d-d15e08bc412b
title: Release 1.0.0-alpha.1, paired with Harrow 0.2.0-alpha.1
type: chore
status: done
milestone: v1.0
assignee: oddurs
created_by: cairn-26
created: 2026-09-28
updated: 2026-09-28
closed_at: 2026-09-28
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
- [x] The tag's release workflow publishes binaries, the source tarball and checksums, marked as a pre-release

## 2026-09-28

Pre-tag checklist from doc/RELEASING.md, run on d26adfa: make check 0, make soak 0, make fuzz 0 (20000 invocations, exit statuses 0/1/2), make conformance 0 (66 cases, two readers agree), make agreement 0 against the pinned Harrow d3ed79c, make dist 0 (cairn-1.0.0-alpha.1.tar.gz), make release-check TAG=v1.0.0-alpha.1 0 ('Cargo.toml, NEWS and Cargo.lock all agree') once the dist tarball was moved out of the tree. PR CI 14/14 green. NEWS dated, with a lead paragraph saying it is an alpha paired with Harrow 0.2.0-alpha.1, and sections for writers waiting their turn, taking turns at breaking an abandoned lock, the quiet broken-pipe exit, and MCP id wording. The release workflow marks a tag with a suffix a pre-release and never latest, so v0.3.0 stays what installers take. Harrow's release is harrow-51's (agreed); it will pin this tag.

## 2026-09-28

Tagged v1.0.0-alpha.1 at 68ac154 (merge of #124) after release-check passed on main. Release run 36476528913 green: preflight, source, five platform builds (aarch64/x86_64 macOS, aarch64/x86_64 Linux musl, x86_64 Windows), publish, crate. GitHub release v1.0.0-alpha.1 is a Pre-release with 7 assets (five binaries, the source tarball, SHA256SUMS); v0.3.0 is still Latest. Verified off the build machine as RELEASING.md asks: downloaded the aarch64-apple-darwin tarball, shasum -c against SHA256SUMS OK, the binary reports 'cairn 1.0.0-alpha.1', gh attestation verify finds 1 attestation from refs/tags/v1.0.0-alpha.1. Not done, deliberately or for want of a secret: no GPG signatures (GPG_PRIVATE_KEY unset) and no crates.io publish (CARGO_REGISTRY_TOKEN unset), both skipped by the workflow as designed; the Homebrew tap left on v0.3.0, since an alpha is not what 'brew install cairn' should give. Harrow 0.2.0-alpha.1 is released by the harrow-51 session against this tag.

## Result

cairn 1.0.0-alpha.1 is released as a GitHub pre-release from tag v1.0.0-alpha.1 (68ac154): binaries for five platforms, the source tarball and checksums, with build provenance attested; v0.3.0 stays the latest release. No signatures or crates.io publish, for want of their secrets.

## 2026-09-28

Homebrew: the harrow-51 session added Formula/cairn-next.rb to oddurs/homebrew-cairn (#3, fixed in #4) at 1.0.0-alpha.1, so brew can install a format-5 cairn; plain cairn stays on 0.3.0. Named cairn-next rather than cairn@1 because Homebrew requires a versioned formula to be keg-only, which would leave no cairn on PATH, and without conflicts_with because tap trust refuses a formula that loads another. Verified there with brew install, brew test and cairn check on a format-5 backlog. doc/RELEASING.md now names both formulae and which a release updates.

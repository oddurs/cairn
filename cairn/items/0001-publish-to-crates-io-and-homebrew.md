---
id: 1
title: Publish to crates.io and Homebrew
type: chore
status: blocked
milestone: later
owner: oddurs
created: 2026-09-04
updated: 2026-09-23
priority: p2
effort: s
area: distribution
---

## 2026-09-05

Blocked on a credential, not on work.

Everything else is done and verified: `cargo publish --dry-run` packages and
compiles the crate cleanly (95 files, 175 KiB compressed), `cairn-md` is still
free on crates.io, and the release workflow's `crate` job is written and skips
itself when no token is present.

To finish, from a machine with a crates.io token:

    cargo login                # or set CARGO_REGISTRY_TOKEN
    cargo publish              # from a clean checkout of the tag

Or add CARGO_REGISTRY_TOKEN to the repository secrets and the existing release
workflow will publish on the next tag without anyone doing anything.

Homebrew, the other half of this item, is done: the tap oddurs/homebrew-cairn is
published and `brew tap` / `brew trust` / `brew install` installs the binary,
the manual page and completions for three shells.

## 2026-09-19

Verified the install path end to end against v0.2.1: `install.sh` detects the platform, downloads `cairn-0.2.1-aarch64-apple-darwin.tar.gz`, verifies the release checksum, installs, and the binary reports `cairn 0.2.1`. So the curl path works today.

crates.io did not publish. The release workflow skips it when `CARGO_REGISTRY_TOKEN` is absent, and the repository has no secrets set at all — `gh api repos/oddurs/cairn/actions/secrets` returns `total_count: 0`. The same is true of `GPG_PRIVATE_KEY`, so v0.2.1 shipped unsigned too (0052). Both are one-time actions only the maintainer can take; nothing in the code is waiting on anything.

## 2026-09-23

Assessment 0134: the binary installer and Homebrew path are already recorded as working. The remaining crates.io publication is an optional distribution choice requiring a maintainer account action. Move it out of doing and out of the historical v0.1 outcome; no credentials were inspected or changed in this assessment.

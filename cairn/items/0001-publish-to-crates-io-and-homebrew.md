---
id: 1
title: Publish to crates.io and Homebrew
type: chore
status: doing
milestone: v0.1
created: 2026-09-04
updated: 2026-09-05
priority: p0
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

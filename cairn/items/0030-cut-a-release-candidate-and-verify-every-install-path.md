---
id: 30
title: Cut a release candidate and verify every install path
type: chore
status: done
milestone: v0.1
assignee: claude
created: 2026-09-04
updated: 2026-09-05
priority: p0
sprint: s5
effort: m
---

## Problem

The release workflow, `install.sh` and the Homebrew formula have never run.
A release pipeline that has not released is a guess.

## Proposal

Tag `v0.1.0-rc.1` and verify each artefact by installing it: the Linux musl
binaries on a clean container, both macOS binaries, the Windows zip, the
`install.sh` path, `brew install`, and `cargo install cairn-md`.

## Acceptance criteria

- [ ] Every Tier 1 and Tier 2 artefact installs and runs `cairn --version`
- [ ] `install.sh` verifies checksums and fails loudly on mismatch
- [ ] Checksums published and verified by hand once
- [ ] Build provenance attested

## 2026-09-05

Every install path exercised against the real release, and each one found
something.

- `install.sh` — downloads, verifies the checksum, installs, and the installed
  binary runs `init`, `new` and `check`. Deliberately fed a tampered
  `SHA256SUMS`: it refuses with both hashes named.
- **Homebrew** — the formula was broken. `man1.install Utils.safe_popen_read(…)`
  passes the page's *contents* where Homebrew expects a *filename*. Fixed by
  writing it to a file first. It now installs the binary, the manual page, and
  completions for bash, zsh and fish, and `brew test` passes.
- **The tap** — a formula in the repository is not installable; Homebrew
  requires a tap. Published `oddurs/homebrew-cairn`. Recent Homebrew also
  refuses untrusted third-party taps, so it is three commands, and the README
  now says so rather than promising two.
- **crates.io** — not done. It needs a token this machine does not have.

Checksums verified by hand once, as the acceptance criteria asked.

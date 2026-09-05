---
id: 53
title: Be in a package manager people already use
type: chore
status: backlog
milestone: v1.0
created: 2026-09-05
updated: 2026-09-05
priority: p0
effort: l
---

## Problem

Installing cairn today means piping a script from the internet, adding a
third-party Homebrew tap, or building from source. All three are things a
careful person hesitates over, and none is how anybody installs the tools they
already trust.

A tool becomes the default partly by being one line in the package manager
somebody already has open.

## Proposal

In rough order of leverage for the effort:

- **crates.io** — the one command every Rust user already knows. Blocked only
  on a token; see `0001`.
- **nixpkgs** — a Rust CLI with a lock file is close to mechanical there, and it
  reaches an audience that cares about exactly the properties this tool has.
- **AUR** — cheap, and Arch users find things.
- **Homebrew core**, once there are enough users to meet the notability bar. The
  tap works meanwhile.
- **Debian and Fedora** are the real prize and the most work; they want a source
  tarball and a signature, which is why `0052` comes first.

Each one gets an entry in the manual and on the site, and each is verified by
installing from it on a clean machine rather than assumed to work.

## Acceptance criteria

- [ ] At least crates.io, nixpkgs and AUR
- [ ] Every path verified by installing on a machine that has never built cairn
- [ ] The install page lists them in the order a reader should try them
- [ ] A release checklist covers updating each one

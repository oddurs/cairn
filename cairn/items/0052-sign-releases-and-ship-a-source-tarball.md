---
id: 52
title: Sign releases and ship a source tarball
type: chore
status: backlog
milestone: v1.0
labels:
- needs-a-key
created: 2026-09-05
updated: 2026-09-06
priority: p0
effort: m
---

## Problem

A release is five binaries and a list of checksums. The checksums come from the
same workflow as the binaries, so they prove the archive downloaded intact and
nothing about who built it. There is no source tarball at all, which is the
form a distribution packager and anybody auditing the thing will ask for first.

For a tool people are asked to keep years of project history in, "you can
verify what you got" is not a nicety.

## Proposal

- A source tarball built from the tag, so packagers have a stable artefact that
  does not depend on the forge staying up.
- Detached signatures over every artefact, with the key and its fingerprint
  published in the repository and on the site.
- Build provenance attestation, which GitHub can emit and which says which
  workflow produced which file.
- Verification instructions somebody can follow without already knowing how.

## Acceptance criteria

- [ ] `make dist` produces the same tarball CI does
- [ ] Every release artefact has a signature
- [ ] The key fingerprint is published somewhere other than the release itself
- [ ] The manual explains verification in steps a first-timer can follow
- [ ] Verified by hand once, from a machine that did not build it

## 2026-09-06

Everything that does not need a key is done.

`make dist` builds a source tarball with `git archive`, which is deterministic
by construction, so the tarball CI attaches and the one you build from the same
tag are the same bytes. Building it with tar(1) instead would mean fighting the
differences between GNU and BSD tar and touch, and losing quietly on somebody
else's machine.

Build provenance is attested by GitHub for every archive. That is the guarantee
the checksums cannot give: they come from the same workflow as the binaries, so
they prove a download arrived intact and nothing about where it came from.

Signing is wired up and inert. The step imports `GPG_PRIVATE_KEY` if the secret
exists and signs every artefact; without it the release proceeds unsigned, so a
fork can still tag. Adding the secret is the whole of what remains.

"Verifying a release" in the manual explains each check, what it proves, and
what it does not — including that the binaries are not reproducible, which
nobody has established and which it would be worse to imply.

## Still open

- A signing key: `GPG_PRIVATE_KEY` and `GPG_PASSPHRASE` as repository secrets,
  and the fingerprint published in the repository and on the site. Publishing
  the fingerprint somewhere other than the release is the part that matters; a
  fingerprint distributed alongside the thing it signs proves nothing.
- Verifying by hand from a machine that did not build it, which cannot be done
  until a release has been made with a key in place.

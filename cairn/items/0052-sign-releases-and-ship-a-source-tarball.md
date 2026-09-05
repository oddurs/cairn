---
id: 52
title: Sign releases and ship a source tarball
type: chore
status: backlog
milestone: v1.0
created: 2026-09-05
updated: 2026-09-05
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

---
id: 120
title: The release checklist cannot hold the promise the release makes
type: chore
status: done
milestone: v1.0
created: 2026-09-12
updated: 2026-09-12
priority: p1
area: release
---

## Problem

`doc/RELEASING.md` opens by saying the document exists so that two people cannot
have different code under one version. The check that would actually prevent
that was a checkbox.

Binary artefacts take their version from the tag (`${GITHUB_REF_NAME#v}` in the
release workflow). The source tarball takes its from `Cargo.toml` (`VERSION` in
the Makefile). Tagging `v0.2.1` against a `Cargo.toml` still saying `0.2.0`
produces one GitHub release containing:

```
cairn-0.2.1-aarch64-apple-darwin.tar.gz   # binary reports `cairn 0.2.0`
cairn-0.2.0.tar.gz                        # the source tarball
```

Two version numbers in one release, and a binary whose `--version` contradicts
the file it arrived in. Nothing stopped it.

Three smaller ones alongside it:

- `workflow_dispatch` on a branch names every artefact after the branch and
  publishes a release for it.
- The release body is GitHub's generated commit list. This project writes NEWS
  carefully and then did not use it.
- `cargo publish` runs only at tag time, after the binaries are already out. A
  packaging failure is discoverable nowhere earlier, and neither is a source
  tarball that does not compile on its own.

## Proposal

`make release-check TAG=v0.2.1`, run by a person before tagging and by the
workflow before it builds anything — one implementation, not two. It refuses a
tag that disagrees with `Cargo.toml`, a missing or still-`(unreleased)` NEWS
section, a stale `Cargo.lock`, and a dirty tree (`git archive` packages HEAD, so
uncommitted work silently does not ship).

`make release-notes` extracts the NEWS section for the release body, with the
generated commit list kept underneath rather than instead.

crates.io moves after the GitHub release rather than beside it: it is the step
that cannot be undone.

CI gains `cargo publish --dry-run` and a build of the source tarball from
scratch in a clean directory, so both tag-time failures surface continuously.

## Acceptance criteria

- [x] A tag disagreeing with `Cargo.toml` is refused before anything is built
- [x] A missing or `(unreleased)` NEWS section is refused
- [x] A stale `Cargo.lock` and a dirty tree are refused
- [x] Dispatching the workflow on a branch is refused
- [x] The release body is the NEWS section for that version
- [x] CI proves the crate packages and the source tarball builds standalone
- [x] The same checks run locally, and `RELEASING.md` says which are automated

## 2026-09-12

Verified each refusal by hand: mismatched tag, missing NEWS section, a section still marked (unreleased), a stale Cargo.lock, and a dirty tree, plus the passing case. `cargo publish --dry-run --locked` and the standalone tarball build were both run locally before being added to CI; the tarball compiles and reports `cairn 0.2.0`.

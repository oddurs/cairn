---
id: 36
title: Publish the item format as a document in its own right
type: docs
status: backlog
milestone: v1.0
created: 2026-09-05
updated: 2026-09-05
priority: p1
effort: s
---

## Problem

The strongest asset this project has is the specification, and it is buried in
chapter eleven of a Texinfo manual almost nobody will open.

Competing on features against a tool with fifty times the users is a losing
game. But nobody has specified what a repository-native backlog file looks
like, and cairn has: a normative chapter, a version, a compatibility policy,
and a corpus proving the reading does not drift.

## Proposal

Extract the format chapter into a standalone document with a stable URL, and
make cairn the reference implementation of it rather than its only definition.

A specification another tool can read is a wider door than a binary another
project has to adopt. It is also the only version of this where being early
counts for more than being polished.

## Acceptance criteria

- [ ] The specification stands alone, without reference to cairn's source
- [ ] Versioned alongside `format`, with the same compatibility promise
- [ ] Linked from the README, the manual and the repository description
- [ ] A second implementation could be written from it — ideally, is

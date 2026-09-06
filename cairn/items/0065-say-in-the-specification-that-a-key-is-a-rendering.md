---
id: 65
title: Say in the specification that a key is a rendering
type: docs
status: backlog
milestone: v0.2
created: 2026-09-06
updated: 2026-09-06
priority: p0
sprint: s6
effort: s
---

## Problem

The specification says `id` is an unsigned integer, and that a reader may take
it from a leading run of digits in the filename when absent. Both remain true
with a project key, but a reader meeting `MP-1002-support-oauth.md` for the
first time will reasonably wonder about both.

This is a clarification, not a change: nothing in the format moves, no key
changes meaning, and the version stays at 1. Getting that distinction right
matters, because the compatibility promise was made a few days ago and its
credibility comes from being applied strictly the first time it is tested.

## Proposal

Two edits.

In §4, note that a project may display identifiers with a prefix, that the
prefix is a property of the project rather than of the item, and that `id` is
the integer regardless of how it is shown.

In §5, say that the leading-digits fallback applies to the default naming, and
that a reader encountering a project with a configured format either parses
through it or requires the `id` key. Recommend the latter for an independent
reader, since the configuration file is explicitly not part of this
specification.

## Acceptance criteria

- [ ] Both sections clarified without any key changing meaning
- [ ] The format version stays at 1, and the reasoning is written down
- [ ] The golden corpus gains a case with a prefixed filename and an `id` key
- [ ] An independent reader can still be built from the specification alone

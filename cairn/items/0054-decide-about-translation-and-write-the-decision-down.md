---
id: 54
title: Decide about translation, and write the decision down
type: docs
status: backlog
milestone: v1.0
created: 2026-09-05
updated: 2026-09-05
priority: p2
effort: s
---

## Problem

A GNU-shaped project is normally translatable. cairn has no message catalogue,
no gettext, and every string compiled in English. That is a real gap against
the tradition it borrows its manners from, and it has never been decided — only
skipped.

## The case against doing it

Most of cairn's output is not labels. It is prose: diagnostics that name a file
and a line, an agent instruction block generated from the schema, error messages
whose whole job is to say precisely what went wrong and what would have worked.
Translating those well is a sustained editorial commitment, not a one-off
extraction, and a half-translated diagnostic is worse than an English one
because it cannot be searched for.

The audience already reads English messages from git, cargo and their compiler.
And a translated error is harder for a maintainer to act on when it appears in a
bug report.

## The case for

It is what the tradition expects, it widens who can use the tool, and the
argument "developers read English" has been used to justify a lot of
parochialism.

## Proposal

Decide, and write the reasoning into the manual either way, so it reads as a
position rather than an oversight. If the answer is no, say what would change
it — a contributor volunteering to own a language, most likely, since the cost
is maintenance rather than extraction.

Recommendation: not before 1.0, and say so plainly.

## Acceptance criteria

- [ ] The manual states the position and the reasoning
- [ ] If no: the condition that would reverse it is written down
- [ ] If yes: a message catalogue and one complete language, not a partial one

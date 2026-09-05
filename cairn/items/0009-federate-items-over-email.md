---
id: 9
title: Federate items over email
type: feature
status: dropped
milestone: later
created: 2026-09-04
updated: 2026-09-05
priority: p2
effort: l
---

## Problem

cairn has no way to accept a change from someone who cannot push. That is the
one thing a hosted issue tracker does that a directory of files does not.

## Proposal

It nearly works already, because items are text files in git: a mailed patch
*is* an issue update. What is missing is making that a first-class path rather
than a coincidence.

    cairn send 12                  # format the item as a patch, hand to git send-email
    cairn apply < message.mbox     # apply an item patch, validating the result

`cairn apply` is where the value is: it validates the incoming item against the
schema *before* applying, so a mailed contribution cannot introduce an unknown
status or a duplicate id.

## Why this and not a forge integration

No account, no server, no API token, and it works with the workflow GNU
projects already run on. It is also the only proposal here that lets a stranger
file an issue without being given commit access.

## Acceptance criteria

- [ ] `cairn send` produces a patch `git am` accepts
- [ ] `cairn apply` refuses a patch that would fail `cairn check`
- [ ] Duplicate ids arriving by mail are reported, with `cairn renumber` as the fix
- [ ] Round trip documented in the manual

## Dropped, 2026-09-05

Elegant, and it presumes contributors who do not exist. Email federation is
how you accept changes from someone without commit access; the project has no
such person, and GitHub issues already serve as the inbox for anyone who
turns up.

Reopen if the project ever outgrows a forge, which is the only situation
where this beats a pull request.

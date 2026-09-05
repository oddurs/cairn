---
id: 45
title: Show an item's history
type: feature
status: backlog
milestone: v0.2
created: 2026-09-05
updated: 2026-09-05
priority: p0
effort: m
---

## Problem

The whole claim is that the repository is the database. A repository gives you
one thing a database does not — history — and cairn never looks at it. There is
no way to ask who changed an item, when, or why it was reopened.

That gap is worse than a missing feature. It is the central claim going
unredeemed: a user who believes "my backlog is in git" will reasonably expect
`cairn log 12` to work, and its absence makes the claim sound like marketing.

## Proposal

    $ cairn log 12
    2026-09-04  oddurs   created
    2026-09-05  claude   status backlog -> doing
    2026-09-05  claude   note added
    2026-09-11  oddurs   status doing -> done

Read `git log --follow` over the item's file — following matters, because
renaming on a title change is a feature and would otherwise break the trail —
and diff the frontmatter between revisions to say what actually changed rather
than showing a patch.

`--patch` for the raw diffs, `--json` for a caller, and a plain fallback when
the project is not a git repository at all, which must not be an error.

## What to be careful about

This is the first command that shells out to git for reading, so it needs the
same discipline as the rest: a project without git still works, a shallow clone
degrades to what it can see and says so, and nothing here becomes a second
source of truth.

## Acceptance criteria

- [ ] Follows renames, so a retitled item keeps its history
- [ ] Reports field changes, not raw patches, by default
- [ ] Works in a shallow clone, and says what it could not see
- [ ] Outside a repository: explains rather than fails
- [ ] `--json` for programs

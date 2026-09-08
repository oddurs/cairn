---
id: 101
title: Be right about data nobody here wrote
type: chore
status: backlog
milestone: v0.2
created: 2026-09-07
updated: 2026-09-07
priority: p1
sprint: s10
effort: l
area: interchange
---

## Problem

Two places take input from outside the project and have to be right about data
nobody in this repository wrote.

**`import.rs`, 683 lines.** It has already shipped a silent data-loss bug: it
matched written items to incoming records by `source`, which a cairn export does
not carry, so every dependency was dropped without a word. That was found by a
round-trip property test rather than by anybody reading the code, which is the
right way round and also a warning about how invisible the failure was.

The GitHub path shells out to `gh` and appears in **four** mentions across the
whole suite. It is effectively untested, and it is the path most likely to meet
real-world data that does not fit: an issue with no title, a label that is not a
string, a body of 400KB, a milestone that means something else there.

**`log.rs`, 568 lines, eight tests.** It parses the output of `git log` — an
external program whose format is stable by convention rather than by contract.
It has produced two real bugs already: `--follow --reverse` silently truncating,
and `--follow` wandering into a *different* item because cairn's item files are
near-identical to each other and git's rename detection is a heuristic.

## Proposal

The review question is the same for both, and it is not "is this correct?" but
**"what does this do when the outside world is wrong?"**

`import`:

- an interchange document with a field of the wrong JSON type, not merely missing
- an item naming a status, type or milestone this project does not have
- two incoming records with the same id
- a body containing the frontmatter delimiter
- `--from github` driven by a recorded `gh` response rather than the network,
  so the path is testable at all: no title, a null body, a label object where a
  string was expected, an issue number that collides with an existing id
- `gh` absent, and `gh` returning non-zero

`log`:

- `git log` output with a commit whose message contains the field separator
- an item whose file was renamed twice in one commit
- a shallow clone, where the history genuinely is not there
- an empty repository

## What this is not

Not a rewrite of either. Both are correct as far as anybody knows; the point is
that "as far as anybody knows" is currently doing more work than it should.

## Acceptance criteria

- [ ] `--from github` is testable without the network, driven by a recorded response
- [ ] Malformed GitHub data is covered: missing title, null body, wrong-typed label, colliding id
- [ ] `gh` absent and `gh` failing both produce a deliberate message
- [ ] An interchange document with wrong-typed and unknown-valued fields is covered
- [ ] `log` is tested against a separator in a commit message, a double rename, a shallow clone and an empty repository
- [ ] Nothing in either path fails by writing a partial result

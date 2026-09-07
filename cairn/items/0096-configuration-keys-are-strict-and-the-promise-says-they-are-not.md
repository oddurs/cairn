---
id: 96
title: Configuration keys are strict, and the promise says they are not
type: docs
status: backlog
milestone: v0.1
depends_on:
- 88
created: 2026-09-07
updated: 2026-09-07
priority: p1
sprint: s9
effort: m
area: config
---

## Problem

`src/config.rs` states the durability promise in full, and the first line of it
is not true of the file it is written in:

> A patch or minor release may add optional keys, and nothing else.

For items it is true, documented in §4.1, and tested: an unknown key survives
being rewritten. For the configuration it is the opposite. `Config` is
`deny_unknown_fields`, so a key added in 0.3 makes every 0.2 refuse the project
outright — same format number, hard failure, no reading at all:

```
$ cairn list
cairn: parsing cairn.toml: TOML parse error at line 3, column 1
unknown field `future_key`, expected one of `name`, `description`, ...
```

Two halves of the same file follow opposite rules, and only one of them is
written down.

## The strictness is right; the promise is wrong

An unknown key in an item came from somewhere — another tool, a newer cairn, a
person — and dropping it loses data. An unknown key in `cairn.toml` is almost
always a typo, and silently ignoring it means a project believes it has
configured something it has not. `deny_unknown_fields` should stay.

What has to change is the sentence, and §8, which currently says a change to the
shape of the configuration alone **may** require a new version. It does not
"may". If the configuration gains a key, every older cairn stops opening the
project, and that is precisely what a format number is for.

## Proposal

Say it, in the three places the promise lives:

- The doc comment on `CURRENT_FORMAT`, which is where a contributor reads the
  rules before changing them.
- §8 of the specification: a new configuration key is a format change, stated
  rather than left as a possibility.
- The manual, in "what would still cost a format number", where the table
  already exists and this line is missing from it.

And improve the error, which currently makes a reader look for a typo without
knowing whether there is one to find. Serde lists the valid keys, which is
useful. What it cannot say is which cairn is reading and which the file expects.
The unknown-key error should name the running version, and suggest the nearest
known key when there is one within an edit or two — so "you misspelled
`criteria_section`" and "this file wants a newer cairn" stop looking identical.

## The cost, acknowledged

Under this rule, adding an optional configuration key is no longer cheap. That
is the point: it was never cheap, it only looked cheap, and every project that
adopted a key we later regretted would have paid for the illusion.

## Acceptance criteria

- [ ] The `CURRENT_FORMAT` doc comment distinguishes item keys from configuration keys
- [ ] §8 states that a new configuration key is a format change
- [ ] The manual's cost table has the line
- [ ] The unknown-key error names the running version
- [ ] It suggests the nearest known key when there is a near miss
- [ ] A test covers a plausible typo and a key from the future producing different advice

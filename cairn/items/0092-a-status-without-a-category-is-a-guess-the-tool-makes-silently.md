---
id: 92
title: A status without a category is a guess the tool makes silently
type: bug
status: backlog
milestone: v0.1
created: 2026-09-07
updated: 2026-09-07
priority: p0
sprint: s9
effort: s
area: config
---

## Problem

`category` is the one key in `cairn.toml` that carries meaning rather than
appearance. It is what makes a project's own vocabulary interchangeable: call a
status whatever you like, and `open` / `active` / `done` / `dropped` underneath
is what every other reader reasons about.

It is also optional, and its default is `open`.

```toml
[[status]]
name = "shipped"
```

```
$ cairn config
statuses
  ...
  shipped      category = open
```

Somebody modelling a terminal state gets the exact opposite of what they meant.
`cairn close` will not target it, progress bars will not count it, the roadmap
will show finished work as outstanding, and nothing anywhere says a word. The
schema is not wrong in a way the tool can see — it is wrong in a way only the
author can see, and the author is the one person who has already stopped
looking.

Every other defaulted key in the file is presentational: a missing `color` is a
colour nobody chose. A missing `category` is a claim about what the status
means, made by the tool, on the author's behalf, silently.

## The wrinkle

Making it required is not free. By the compatibility rules, making an optional
key required needs a new format number and a migration — the rule exists for
exactly this reason, and it should not be waived because this instance feels
obvious. Format 3 is not worth spending on one key.

So the fix now is a warning, and the requirement rides along the next time a
format number moves for a reason that earns one.

## Proposal

`cairn check` reports a status that does not declare a category, naming it and
saying what it was assumed to be. It is a warning rather than an error: an
existing project must not stop working over this.

`cairn config` shows an assumed category differently from a declared one, so the
resolved schema does not present a guess as a decision.

The manual says why this one key has no honest default, and the entry in "what
would still cost a format number" gains a line for making it required.

## Acceptance criteria

- [ ] `cairn check` warns for each status with no declared category, naming what was assumed
- [ ] It is a warning, not an error: a project that has one still works
- [ ] `cairn config` distinguishes an assumed category from a declared one
- [ ] The manual says why `category` is the key that should not have a default
- [ ] "What would still cost a format number" records making it required

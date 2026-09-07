---
id: 94
title: A renamed milestone field empties the roadmap without a word
type: bug
status: done
milestone: v0.1
created: 2026-09-07
updated: 2026-09-07
priority: p1
sprint: s9
effort: s
area: refs
---

## Problem

Rename the milestone field and the roadmap disappears without a word.

```toml
[[field]]
name = "release"     # was "milestone"
kind = "ref"
target = "milestone"
by = "key"
rollup = true
```

Items updated to match. Then:

```
$ cairn check
ok: 4 item(s), 0 warning(s)
$ cairn roadmap
T
$
```

The project name, and nothing. `Milestones::new` looks up `cfg.field("milestone")`
by literal name (`src/refs.rs:450`) and returns an empty set when it is not
there. `render.group_by` still defaults to `"milestone"` as well, so the
rendered roadmap goes the same way.

The schema is internally consistent. Every reference resolves. `check` is clean.
And the command that exists to draw the roadmap draws nothing and reports
success.

## Two problems wearing one coat

**A command with nothing to show should say why.** This is the smaller half and
the one worth fixing first. `cairn roadmap` with no container type to group by
is not an empty roadmap, it is a question the schema does not answer, and
printing the project name is the worst available answer. The same is true of
`cairn board` with no visible statuses.

**Field names are promises, in the implementation, today.** This is the larger
half, and it is the live evidence for the decision recorded in `0089` — which
concluded, correctly, that a role mechanism is not worth building yet. That
conclusion does not survive the tool pretending the name is free when it is not.
`milestone` is looked up by literal string in `refs`, `list`, `show`, `import`
and `init`, and the default `render.group_by` is the same string.

So: either the name is load-bearing and the tool says so, or it is not and the
lookup goes through the schema. The first is a day's work and honest. The second
is `0089`, and `0089` says not yet.

## Proposal

`cairn roadmap` and `cairn board` report when they have nothing to draw and why
— no field targets a container type, or no status is visible — rather than
printing a heading over silence.

`cairn check` warns when `render.group_by` names a field that does not exist,
which is the same defect arriving by a different road (see `0093`).

The manual's account of milestones says plainly that `milestone` is the name the
tool looks for, and points at the roles section for why that is a decision
rather than an oversight.

## Acceptance criteria

- [x] `cairn roadmap` with no container type says so, rather than printing a bare heading
- [x] `cairn board` with no visible status says so
- [x] Neither is an error: an empty project is not a broken one
- [x] The manual states that `milestone` is looked up by name, next to the roles decision
- [x] A test renames the field and asserts the tool explains itself

## 2026-09-07

Done, both halves. `cairn roadmap` and `cairn board` say why they have nothing to draw, and `cairn check` reports `render.group_by = milestone` with no field of that name. The manual now states plainly, next to the roles decision in 0089, that cairn looks the name up literally — the honest version of that decision rather than the quiet one.

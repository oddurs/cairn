---
id: 78
title: Decide whether milestones should be items
type: docs
status: backlog
milestone: v1.0
depends_on:
- 72
created: 2026-09-06
updated: 2026-09-06
priority: p0
effort: m
sprint: s8
---

## Problem

Once items compose (`0072`), a milestone is an item with a due date and things
part of it. Keeping both mechanisms means keeping the worse copy.

Milestones are declared in `cairn.toml`, and everything they cannot do follows
from that. A milestone has no body, so it cannot carry the reasoning every other
object in cairn carries. It has no history, so `cairn log` cannot show when a
date moved or who moved it — which is the single most interesting question about
a milestone. It cannot depend on anything, cannot be assigned, cannot be
searched, and cannot be closed with criteria.

It also needed a rule of its own. `milestones_ordered` walks the list backwards
so that an undated milestone inherits the date of the next dated one. That code
exists only because milestones are a lesser kind of thing with no natural order;
items have one.

## The change

`type = "milestone"`, a `due` field, and items `part_of` it. Then:

- a milestone gets a body, and the reason for a date lives with the date
- `cairn log v0.2` answers when it slipped and who slipped it
- the bespoke ordering rule is deleted
- `cairn roadmap` becomes a view over the graph rather than a separate renderer
- one concept fewer in the manual, in `cairn.toml`, and in the schema an agent
  has to learn

## The cost, priced honestly

This changes what a key means, so by this project's own compatibility rules it
is **format 2, a major release, and a migration**. Those rules were written days
ago and their credibility comes from being applied strictly the first time they
are inconvenient. This is that time.

There is a second cost that matters more than it looks. `milestone: v0.1` in the
frontmatter is a name a person reads; `part_of: [42]` is not. The
human-readability of an item file is one of the reasons the format is worth
having, and trading it for an internal consistency win is not obviously right.

Mitigations exist and each has a wrinkle. Keep `milestone:` as a rendered view
over the relation, and there are two ways to say one thing. Let relations
reference by a stable name rather than an id, and the identifier design in
`0062` has to answer what a name is. Neither is free.

## Recommendation

Decide before 1.0, ship after. Deciding is cheap now and the decision governs
`0072`'s design; shipping it means a migration and a major version, which should
happen once, deliberately, alongside anything else that needs one.

My inclination is to do it, on the grounds that the concept is genuinely
redundant and the ordering wart is evidence of it. I hold that loosely, because
the human-readability cost is real and I have not found a mitigation I like.

## Acceptance criteria

- [ ] The decision is recorded either way, with the reasoning
- [ ] If yes: what `milestone:` becomes in the file, and whether a name survives
- [ ] If yes: the migration is written and tested before the format bumps
- [ ] If no: what happens to the ordering rule, and why the duplication is worth
      keeping
- [ ] Somebody other than the author reads the argument first

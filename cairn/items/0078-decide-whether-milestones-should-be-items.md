---
id: 78
title: Decide whether milestones should be items
type: docs
status: backlog
milestone: v1.0
depends_on:
- 79
created: 2026-09-06
updated: 2026-09-07
priority: p0
effort: m
sprint: s8
---

## Decided: yes, and not the way this item first proposed

The original proposal was that a milestone becomes an item you are `part_of`.
That is wrong, and the reason is worth keeping because it is the kind of mistake
that looks like simplification.

**`milestone` is single-valued. `part_of` is many-to-many.** An item belongs to
several initiatives and ships in exactly one release. Collapsing them does not
remove a redundant concept; it discards the constraint that makes a milestone
mean anything. If `part_of` absorbed it, nothing would stop an item being in
v0.1 and v0.2 at once, and the roadmap would have no answer for which.

So milestone is a field, and it stays a field. What was actually wrong was never
that — it was **where a milestone's own information lives**.

## What is actually wrong

`cairn.toml` cannot hold a body and cannot be asked when something changed. So a
milestone, alone among the things cairn manages:

- has no reasoning. "Why is v0.2 due in December" has nowhere to go, in a tool
  whose whole argument is that items carry the thinking that produced them.
- has no history. `cairn log v0.2` — when did this date move, and who moved it —
  is the most interesting question about a milestone, and is unanswerable.
- cannot be assigned, searched, closed with criteria, or depended upon.
- needed a rule of its own. `milestones_ordered` walks the list backwards so an
  undated milestone inherits the date of the next dated one. That code exists
  only because milestones have no natural order. Items have one.

None of that is a consequence of milestone being a field. All of it is a
consequence of the milestone itself not being an item.

## The change

`cairn.toml` stops declaring milestones. A milestone is an item:

```yaml
---
id: 42
type: milestone
key: v0.1
title: Usable in anger
due: 2026-10-15
status: doing
---

Enough to run a real project's roadmap without reaching for anything
else. October because the conference is in November and this is the
thing worth showing there.
```

And `milestone` becomes a ref field (`0079`):

```toml
[[field]]
name        = "milestone"
kind        = "ref"
target      = "milestone"
cardinality = "one"
by          = "key"
rollup      = true
```

**Item files do not change.** `milestone: v0.1` in the frontmatter means the
same thing it means today and is spelled the same way. That is the whole reason
for `by = "key"`.

## Compared with what this item first proposed

|  | first proposal | this |
| --- | --- | --- |
| Item files change | every one | **none** |
| `milestone: v0.1` in a file | becomes `part_of: [42]` | unchanged |
| Closed vocabulary, typos caught | weakened | kept, by target type |
| An item can be in two milestones | yes, wrongly | no |
| Migration touches | every item | `cairn.toml` |
| Reasoning and history | gained | gained |

## Ordering, which does not disappear

Claiming the ordering wart is deleted would be too neat. It moves, and it gets a
better answer.

Milestone items are items, so they can depend on each other, and a roadmap is a
sequence: v0.2 depends on v0.1, `later` depends on v1.0. Order is then a
topological sort of that graph, with `due` breaking ties and identifier order
breaking what remains. An undated milestone sits where its dependencies put it,
which is what the backwards-walking inheritance rule was approximating badly.

`cairn milestone add` wires the new one after the last by default, so nobody has
to think about it.

This works only because container types are absent from `cairn next` (`0079`).
Otherwise milestones depending on each other would read as blocked work and
crowd out the real thing.

## What it costs

The item format does not move: §7 of the specification says the configuration
format is not part of it, and no item file changes. But the format number lives
in `cairn.toml` and governs on-disk shape, so this is **format 2, a major
release, and a migration** — of one file per project, mechanically, from
`[[milestone]]` blocks to item files.

Those rules were written days ago and their credibility comes from being applied
strictly the first time they are inconvenient. This is that time, and the honest
answer is that it costs a major version even though it is cheap.

A pleasing consequence: `cairn migrate` is hidden from `--help` today, with a
test asserting `CURRENT_FORMAT == 1` and instructing whoever changes that to
unhide it. This is the change that gives the command something to migrate. The
rule wrote down its own first use before there was one.

## Loose ends this creates

**`cairn milestone` becomes doubtful.** Once milestones are items, `milestone
add` is `new -t milestone` and `milestone list` is `list -t milestone`. By the
bar in `0051` — removing it must make a real task *harder* — it probably does
not survive. That is a good outcome, and it is a second breaking change to
sequence with this one rather than discover afterwards.

**`[[milestone]]` in `cairn.toml` had a property worth naming.** It was a closed,
declared vocabulary: a milestone could not be conjured by typo. `target =
"milestone"` keeps that, because a value must name an existing item of that
type. Worth stating explicitly in the migration notes, because it is the thing
somebody will fear losing.

## Acceptance criteria

- [ ] `0079` lands first; this is built on it and not beside it
- [ ] `milestone:` in an item file is unchanged, before and after
- [ ] `[[milestone]]` is gone from `cairn.toml`, and `cairn init` scaffolds items
- [ ] The migration is written and tested against a real project before the
      format number moves
- [ ] Migrating twice is a no-op, and migrating an already-migrated project says so
- [ ] `milestones_ordered` and its inheritance rule are deleted, and roadmap
      order comes from dependencies, then `due`, then identifier
- [ ] `cairn log v0.1` answers when the date moved
- [ ] A milestone with no items still renders on the roadmap
- [ ] `cairn migrate` is unhidden, and the test in `tests/rules.rs` updated
- [ ] The fate of the `milestone` command is decided in the same release

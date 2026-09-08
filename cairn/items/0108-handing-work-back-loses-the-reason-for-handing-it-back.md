---
id: 108
title: Handing work back loses the reason for handing it back
type: feature
status: done
milestone: v0.1
created: 2026-09-08
updated: 2026-09-08
priority: p1
sprint: s11
effort: s
area: agents
---

## Problem

`cairn release` takes `--status` and `--keep-status`. It does not take a reason.

```
Options:
  -s, --status <STATUS>  Status to move back to
      --keep-status      Keep the current status, only drop the assignee
```

So an agent that spends an hour discovering an approach does not work, gives up,
and hands the item back, hands back **only the item**. Tomorrow another agent
takes it and walks the same dead end, because the most valuable thing the first
one learned was never written down.

This is a general problem with a particular sting for agents. A person who
releases something usually says so in passing --- in a message, at a standup, to
the person who asks. An agent has no passing. If it is not in the item, it did
not happen.

## Proposal

```
cairn release 12 --reason "The parser rewrite needs the format decision in 0067 first."
```

The reason is appended to the body as a note, under a dated heading, in the
releaser's own name. Not a frontmatter field: it is prose, it is one of possibly
several, and the body is where every other reason in this project already lives.

The same on `release_item` over the protocol, and the note should be the first
thing `claim` shows the next taker --- an item handed back with a reason is a
different proposition from one nobody has touched, and the next agent should
know which it has.

`cairn next` should mark an item that has been released with a reason, for the
same purpose: choosing what to do is better informed by "somebody tried this"
than by priority alone.

## Why not a field

`released_because` would be one string, overwritten by the next releaser, and
would go stale the moment somebody made progress. Three attempts on a hard item
is a history, and a history is what a body is for.

## Acceptance criteria

- [x] `cairn release --reason` appends a dated note naming who released it
- [x] The protocol server exposes the same, since an agent is the likeliest caller
- [x] `cairn claim` shows the most recent release reason when there is one
- [x] `cairn next` marks an item previously released with a reason
- [x] Releasing without a reason still works and says nothing extra
- [x] Nothing is stored in frontmatter: three attempts is a history, not a field

## 2026-09-08

Done. `release --reason` appends a dated note in the releaser's name, and `claim` shows the most recent one to whoever takes the item next. A note rather than a field, as filed: three attempts on a hard item is a history. `release_item` is a twelfth protocol tool, described so a model reaches for it rather than leaving an item claimed when it cannot finish. The appending logic was implemented separately in `note` and in the protocol server; it is now `Item::append_note`, used by all three.

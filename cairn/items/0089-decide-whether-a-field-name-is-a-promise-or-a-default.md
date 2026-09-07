---
id: 89
title: Decide whether a field name is a promise or a default
type: docs
status: backlog
milestone: v1.0
depends_on:
- 88
created: 2026-09-07
updated: 2026-09-07
priority: p2
effort: m
sprint: s9
---

## Decision, not a task

Every documented key is a name. `assignee`, `milestone`, `depends_on`. A project
that calls its people *maintainers* still writes `assignee:`, and a project with
no releases still writes `milestone:` or nothing at all.

That is a promise about vocabulary, and vocabulary is the thing most likely to
be wrong for somebody. Changing one costs a format number.

There is a mechanism that would make it not cost one, and this item is for
deciding whether it is worth having.

## The proposal being weighed

A field declares the **role** it plays, and the role is what the format
promises:

```toml
[[field]]
name = "maintainer"
role = "assignee"      # this project's word for the assignee role
```

The format documents a small closed set of roles --- identity, title, state,
scheduled-for, blocked-by, composed-into, worker, owner, provenance --- and a
project maps its own names onto them. A consumer reasons about roles. Renaming
a field becomes a configuration change rather than a format change.

It is the status-category trick one level up: a fixed axis underneath, chosen
names above.

## The case against, which is strong

**It costs the thing that makes these files worth having.** Today anybody can
`grep 'assignee:'` across every cairn project in the world and get the right
answer. Under roles they would have to read a configuration file first to learn
what this project calls it. A format whose meaning requires a lookup is a worse
format, and §4.3 already leans the other way by saying a reader that ignores the
project's rendering is conforming.

**Nobody has asked.** The names have not been wrong for anybody yet, because
there is not yet an anybody. Building a mechanism for a problem that has not
occurred is how a schema becomes a framework.

**It is not needed to keep the option open.** A role attribute is a new optional
key, so adding it later is free under the compatibility rules. This is one of
the rare decisions that does not get more expensive by waiting.

## The version that might be right

Keep the names as **defaults** rather than promises. A field called `assignee`
plays the assignee role without saying so; a project that wants a different word
declares the role. Grep keeps working for everybody who never touches it, and a
rename never costs a format number.

That is additive, so it can happen at any time, which is precisely the argument
for not doing it now.

## Recommendation

Not now, and not never. Write the roles down in the manual as a named idea with
this reasoning, so the next person who wants to rename a field finds an answer
rather than an absence — and revisit it the first time somebody's vocabulary
genuinely does not fit.

The condition to watch for: a real project whose word for one of these is wrong
enough that its people keep having to translate.

## Acceptance criteria

- [ ] The decision is recorded either way, with the argument on both sides
- [ ] If no: the condition that would reverse it is named
- [ ] It is stated that adding roles later is free, so waiting costs nothing
- [ ] Somebody other than the author reads it before it is settled

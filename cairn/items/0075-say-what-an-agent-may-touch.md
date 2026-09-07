---
id: 75
title: Say what an agent may touch
type: feature
status: backlog
milestone: v1.0
created: 2026-09-06
updated: 2026-09-06
priority: p0
effort: m
sprint: s8
---

## Problem

`cairn agent` and the MCP `get_schema` tool tell a model exactly which statuses,
types and fields exist. That closed vocabulary is the most useful thing cairn
does for an agent, and it is only half the answer: it says what *exists*, never
what the agent *may touch*.

So an agent that can write at all can write anything. It can reprioritise the
backlog, move items between milestones, rewrite acceptance criteria, and close
work nobody asked it to close. Nothing in the schema lets an owner say
otherwise.

This is the thing people are actually nervous about. A tool that says "agents
may set status and add notes; they may not change priority, milestone, or the
criteria" is one they will let near a real backlog, and that trust is a stronger
differentiator than any feature on the list.

## Proposal

Per field, in the schema that already describes it:

```toml
[[field]]
name = "priority"
agent = "read-only"       # default: "write"
```

And per status, for the transitions that matter most:

```toml
[[status]]
name = "done"
agent = "propose"         # an agent may ask, and a person confirms
```

`propose` needs somewhere to put the proposal. The body is the obvious place —
an agent appends a note saying what it believes and why — which costs no new
concept and leaves the reasoning where every other reason lives.

## Where this can and cannot be enforced

Honestly, and in the manual: **this is a guard rail, not a security boundary.**

The MCP server knows who is calling — `clientInfo.name` is already the caller's
identity — so it can refuse, and that is the path agents actually use. The
command line cannot: an agent with a shell can run `cairn set`, and pretending
otherwise would be worse than not offering it. `git` hooks are the right
comparison, and the manual should make it exactly.

Claiming more than that would be the first dishonest thing in this project's
documentation.

## Acceptance criteria

- [ ] A field can be marked read-only for agents, and the MCP server refuses it
- [ ] A status transition can require a person, and refusal explains why
- [ ] The refusal names what to do instead, rather than only saying no
- [ ] `cairn agent` reports the restrictions, so a model learns them before trying
- [ ] The manual says plainly this is a guard rail, not a boundary
- [ ] A project that configures nothing behaves exactly as it does today

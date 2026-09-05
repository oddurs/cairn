---
id: 37
title: Point a real agent at the MCP server and watch it work
type: chore
status: done
milestone: v0.1
assignee: claude
created: 2026-09-05
updated: 2026-09-05
priority: p0
effort: s
---

## Problem

The thesis is that agents work the backlog through tools rather than prose. The
MCP server is verified only by protocol tests written by the same person who
wrote the server. It has never been wired into a client and driven by an agent
doing an actual task.

That is the central claim, and it is the least tested thing here.

## Proposal

Wire `cairn mcp` into a real client, give an agent a real task in a real
project, and watch. Record what it reaches for, what it gets wrong, and which
tool descriptions mislead it.

The output is not a passing test. It is a list of the places where the tools
say something other than what an agent needs to hear.

## Acceptance criteria

- [ ] An agent finds work, claims it, does it and closes it without being told
      the commands
- [ ] It never writes a TODO or plan file alongside the backlog
- [ ] Every tool description that misled it is rewritten
- [ ] Whatever it could not do through tools is filed

## 2026-09-05

Driven end to end as a client, in a project with four items and a dependency.
The loop works: `get_schema` gives statuses, fields, counts and the filter
grammar; `next_items` offers only ready work and never the blocked item;
`claim_item` with no id takes the top-ranked one and returns its body so work
can start; `add_note`, `close_item`, `check` all behave. Closing the blocker
made the dependent item appear on the next call, unprompted.

Errors are actionable, which was the point of putting them in-band: a status
label instead of a name comes back with the six valid names, an undeclared
field comes back with the declared ones, and a blocked claim names what blocks
it.

## What it found

An agent's work was recorded under `git config user.name` — the repository
owner's — because identity fell through to `whoami`. That is both wrong and the
kind of wrong nobody notices, since the backlog simply looks like the human did
it. The protocol already carries the answer: `clientInfo.name` from
`initialize`. Now `CAIRN_USER` wins if set, then the client's own name, then
the login.

The server instructions gained the thing the failure modes suggested an agent
would get wrong first: a status is named by the project, and `update_item`
wants the name, not the label it was shown.

## What this does not prove

A model *choosing* these tools unprompted, in a real client session, remains
untested. This drove the protocol; it did not test whether the descriptions lead
a model to reach for the right tool. That needs cairn wired into a client, and
is worth doing before anyone relies on the agent story.

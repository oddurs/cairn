---
id: 39
title: Record a decision on an item without rewriting the file
type: feature
status: backlog
milestone: v0.2
created: 2026-09-05
updated: 2026-09-05
priority: p1
effort: s
---

## Problem

Found while dropping four items and writing down why. There is no way to add to
an item's body from the command line. `new --body` sets one at creation, `set`
does not touch the body at all, and the MCP `update_item` tool replaces it
wholesale. Recording a decision meant appending to the file with a shell
redirect, going around the tool entirely.

That is a hole in exactly the place this project cares about. An agent that
decides not to do something can change the status but cannot say why — and a
status without a reason is how a backlog becomes a list of things nobody
remembers rejecting.

## Proposal

    cairn note 12 "Dropped: libguile ends the self-contained binary."
    cairn note 12 --heading "Dropped, 2026-09-05" --stdin < reasoning.md

Appends to the item's body under a dated heading, leaving what is already
there alone. The same as an MCP tool, since this matters most for callers that
have no shell.

Whether `set` should learn `body+=` instead is worth deciding first; a separate
verb reads better at a terminal and is harder to invoke by accident.

## Acceptance criteria

- [ ] Appends without disturbing existing body content
- [ ] Available over MCP as an append, distinct from replacing the body
- [ ] Reads from stdin, so an agent can write more than a line
- [ ] Dropping an item with a reason takes one command

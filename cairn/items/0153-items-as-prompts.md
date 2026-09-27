---
id: 153
uid: 835b4f34-ebe7-44b8-b8f7-3055866aaad0
key: prompts
title: Items as prompts
type: milestone
status: backlog
depends_on:
- 135
created: 2026-09-27
updated: 2026-09-27
priority: p2
---

An item reads as a prompt that outlives the session, and cairn treats it as
one: it compiles an item and everything it rests on into what an agent should
read, carries a finished item's result to the work that depends on it, splits
a prompt too large to run into sub-prompts, and says when a prompt will be
misread.

## Release gate

Every command in this milestone works on this repository's own backlog, and an
agent starting an item can begin from `cairn prompt <ID>` alone. No runner, no
service: each piece is a view, a convention or an ordinary write over the
record that already exists (0142).

## Sequence

The decision first, then Results, because the compiler quotes them; then the
compiler; then split and the prompt checks, which the compiler also surfaces.

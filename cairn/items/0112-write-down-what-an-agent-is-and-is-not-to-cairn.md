---
id: 112
title: Write down what an agent is, and is not, to cairn
type: docs
status: backlog
milestone: v1.0
depends_on:
- 106
- 107
- 108
- 109
created: 2026-09-08
updated: 2026-09-08
priority: p2
sprint: s11
effort: s
area: docs
---

## Problem

cairn treats an agent as *a user who happens to be a program*, and that premise
produced most of what is good about the agent surface: schema first, `next` as
the primitive, claims against duplicate work, permissions that are enforced,
notes that append rather than replace, an instruction block generated from the
real schema so it cannot drift.

The premise is right about writing and wrong in three specific ways, and nothing
in the documents says which three. So each decision about agents gets argued from
scratch, and the next one will be argued from scratch again.

## The three ways, which are worth writing down

**A person's memory outlives the session; an agent's does not.** The backlog is
not the agent's record, it is its memory. That is why filing a duplicate is the
characteristic agent failure (`0107`) and why a reason for handing work back has
to be in the item or it did not happen (`0108`).

**A person's attention is free; an agent's is metered.** Every character returned
is charged to the caller. A person skimming a table pays nothing for the columns
they ignore. This is why a read path that always returns everything is not a
minor inefficiency but a design error (`0109`).

**A person notices they have stopped; an agent ceases to exist.** Every operation
that takes a lock on intent assumes the holder comes back. That is why a claim
needs a date and a way to be seen (`0106`).

## What follows from them, including the refusals

The same three also say what *not* to build, which is the more useful half:

- **No parallel agent API.** The value is that agents and people share one
  backlog. Two views drift, and the shared artefact stops being shared.
- **No memory store.** The items are the memory. Anything else is the TODO file
  the instruction block already forbids.
- **No leases or heartbeats.** A distributed-systems answer to a filesystem
  problem, requiring something to be running --- which is exactly what cannot be
  assumed about the thing that stopped.
- **Nothing that calls a model.** cairn is read and written by programs that
  think; it does not think.

## Proposal

A chapter in the manual --- beside "Coding agents", which currently says how to
point one at a project but not what cairn believes about them --- stating the
three differences and the four refusals.

Not a specification change. This is about the tool's posture, not the format,
and §7 is right that a reader of items needs to know nothing about any of it.

## Why bother

Because the four refusals are each individually tempting, and the argument
against them is the same argument every time. Written once, the next person who
wants an agent-shaped API can disagree with the reasoning instead of relitigating
it, which is the whole reason this project writes decisions down.

## Acceptance criteria

- [ ] A manual chapter stating the three ways an agent differs from a person
- [ ] Each is tied to the decision it produced, by item number
- [ ] The four refusals are stated with their reasons
- [ ] It says plainly that this is posture, not format: `spec/README.md` is unchanged
- [ ] `cairn agent` is unchanged --- this is for the person choosing, not the model working

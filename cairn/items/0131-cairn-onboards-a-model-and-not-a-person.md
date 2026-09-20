---
id: 131
title: cairn onboards a model and not a person
type: feature
status: backlog
milestone: v1.0
created: 2026-09-19
updated: 2026-09-19
priority: p2
area: cli
effort: s
---

## Problem

`cairn agent --write AGENTS.md` generates, for a language model: what the project
tracks, where items live, the loop to work in, the commands, the types, the
statuses, every field with its permitted values, the saved views, and four rules.
It is regenerated in CI so it cannot drift.

A person who clones the same repository gets nothing equivalent. Not that the
project tracks work in cairn, not how to install it, not what `sprint = s7` means
here, not that `cairn check` must pass before a pull request.

That asymmetry looks accidental rather than decided. The same schema is already
being read and rendered; only the audience is missing.

## Proposal

Unclear, and that is why this is an item rather than a change.

The easy version is a flag — `cairn agent --for human`, writing the same content
without the model-directed framing. Cheap, and probably wrong: a block generated
into a README is a block that fights whatever the README already says, and
CONTRIBUTING is a document with a voice rather than a schema dump.

The version worth considering is narrower: the part a newcomer cannot get
anywhere else is *this project's vocabulary* — its statuses and what they mean,
its fields and their values, its views. That is what `cairn config` already
prints, and the question is whether anything needs generating at all, or whether
the answer is a line in the README saying `cairn config` is the schema and
`cairn --help` is the rest.

Against doing anything: README and CONTRIBUTING are human onboarding, every
project writes them, and cairn generating prose into them is the kind of surface
that seems reasonable when written and is regretted later. The four rules in
CONTRIBUTING exist to catch exactly that.

For doing something: the model gets a generated, always-current, complete
description and the person gets whatever somebody remembered to type. If the
generated block is worth having for one reader it is hard to argue it is
worthless for the other.

## Decide first

- Is a newcomer missing anything `cairn config` and `cairn --help` do not already
  give them, once they know cairn is in use at all?
- If the answer is only "they do not know cairn is in use", that is one line in a
  README and not a feature.
- Does `cairn init` mentioning it in the README it does not write solve this
  better than any generated output?

## Acceptance criteria

- [ ] Decided, either way, with the reasoning written down
- [ ] If nothing is built, this item says why, and says it well enough that it is not reopened

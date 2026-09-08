---
id: 109
title: Reading costs the caller and there is no way to ask for less
type: feature
status: done
milestone: v0.2
created: 2026-09-08
updated: 2026-09-08
priority: p1
sprint: s11
effort: m
area: mcp
---

## Problem

Every read tool returns everything cairn knows about an item. Measured on a real
project, `list_items` costs **574 characters per item**, carrying 21 keys:

```
assignee blocked blockers category created created_by depends_on fields
id key labels milestone owner path ready ref source status title type updated
```

An agent asking *what should I do next* needs `id`, `title`, `status`,
`priority`, `blocked`. For fifty items it is spending about 29KB of context to
answer a question that needs three.

A person skimming a table pays nothing for the columns they ignore. Context is
the one resource an agent cannot get more of, and cairn currently spends it on
the caller's behalf without asking.

## Proposal

A `fields` argument on the read tools --- `list_items`, `next_items`,
`search_items`, `show_item`:

```json
{"name": "next_items", "arguments": {"fields": ["id", "title", "priority", "blocked"]}}
```

Absent, nothing changes: the default stays exactly what it is today, because
changing a default is how you break every consumer at once.

The tool descriptions should say what the cheap shape is, since the model
choosing the arguments is the one paying for the answer, and it will not think
to ask unless told it may.

The command line already has this in `--columns`. This is that idea, at the
boundary where it costs money.

## The related half

`get_schema` should say what fields exist and roughly what a full item costs, so
a caller can decide before asking rather than after paying. One sentence in the
schema is cheaper than fifty items nobody needed.

## Acceptance criteria

- [x] `fields` on `list_items`, `next_items`, `search_items` and `show_item`
- [x] Absent, the shape is byte-identical to today's
- [x] An unknown field name is an error naming what is available, not silence
- [x] `id` is always present whatever is asked for, since a result nothing can be done with is worthless
- [x] The tool descriptions tell a model it may ask for less
- [x] A test asserts a narrow request is materially smaller than the default

## 2026-09-08

Done. `fields` on list_items, next_items, search_items and show_item; asking for four of twenty-two keys is 78% smaller, measured. The request is validated once against the schema rather than per item, so a milestone with no priority answers null instead of failing the whole call — the first implementation got that wrong and a test caught it. A schema field can be asked for by its own name even though cairn keeps it under `fields`, because where cairn files it is cairn's business. `id` always survives. Absent, the shape is byte-identical.

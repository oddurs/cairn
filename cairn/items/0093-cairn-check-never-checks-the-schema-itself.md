---
id: 93
title: cairn check never checks the schema itself
type: feature
status: done
milestone: v0.1
created: 2026-09-07
updated: 2026-09-07
priority: p1
sprint: s9
effort: m
area: check
---

## Problem

`cairn check` validates every item against the schema. Nothing validates the
schema against itself, and `Config::validate` stops at structure: unique names,
enum values present, `target` only on refs, `default_status` exists.

So a schema can be thoroughly wrong and pass:

```toml
[render]
group_by = "epic"          # there is no `epic`
```
```
$ cairn check
ok: 4 item(s), 0 warning(s)
$ cairn render
$ echo $?
0
```

A saved view is the same. Its filter is a string until somebody uses it:

```toml
[[view]]
name = "bad"
filter = "stauts=todo"
```
```
$ cairn check
ok: 4 item(s), 0 warning(s)
$ cairn list --view bad
no items match
```

"No items match" is the truth and the wrong answer. The view is not empty, it is
broken, and the message sends the reader to look at their backlog instead of at
the typo.

The asymmetry is the point: an item with a status the schema does not define is
reported immediately, but a schema referring to a field that does not exist is
nobody's problem until it quietly produces the wrong output.

## Proposal

`cairn check` checks the configuration first, before it looks at a single item,
and reports what it finds the way it reports everything else.

What is worth checking, and all of it is cheap:

- `render.group_by` names a field, a reserved key, or a ref that exists
- every `[[view]]` filter parses, and every field it names is declared or reserved
- every `[[view]]` `sort` and `columns` entry names something real
- `render.include` parses
- `render.header` and `render.footer` name files that exist
- `render.link_items` is set but `project.url` is not

A filter naming an *undeclared* field is a warning rather than an error, because
item frontmatter is open-ended and querying a key nothing carries is legal. A
filter that does not parse is an error: it cannot be what anybody meant.

## Why here rather than at load

Because a broken view must not stop `cairn show` from working. Load-time
validation is for a schema cairn cannot operate at all; this is for a schema
that operates and misleads. `check` is where a project asks whether it is in
good order, and this is part of that question.

## Acceptance criteria

- [x] `cairn check` validates the configuration before the items
- [x] `render.group_by`, `render.include`, `header` and `footer` are checked
- [x] Every saved view's filter, sort and columns are checked
- [x] A filter that does not parse is an error; an undeclared field is a warning
- [x] `link_items` without `project.url` is reported
- [x] None of it prevents a read command from working

## 2026-09-07

Done. `cairn check` validates the schema before it looks at an item. The list of derived keys moved to `filter::DERIVED_KEYS`, beside `resolve`, so adding one adds it to both and a working saved view is never reported as a typo. Diagnostics carry a line number from a best-effort scan of the raw file, since serde does not keep spans.

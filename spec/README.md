# The cairn item format

**Version 1.** A specification for storing a project's backlog as files in its
own repository.

This document is normative and stands alone: a reader for the format can be
implemented from it without consulting any particular program. [cairn][] is the
reference implementation, not the definition.

[cairn]: https://github.com/oddurs/cairn

---

## 1. Why a specification

A backlog kept in a repository outlives the tool that wrote it. Somebody must be
able to read those files in ten years, with whatever software exists then, and
know what they meant. That is only true if the format is written down.

It also means a project is not required to adopt one program to adopt the
convention. Anything that can read Markdown and YAML can read a cairn backlog,
and anything that can write them can contribute to one.

## 2. Terminology

The key words **must**, **must not**, **should**, and **may** are to be
interpreted as in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119).

An **item** is one unit of tracked work. A **project** is a directory
containing a configuration file and a directory of items.

## 3. Structure of an item

An item is a file whose name ends in `.md`, containing, in order:

1. A line consisting of exactly `---`, optionally preceded by a UTF-8 byte order
   mark.
2. Zero or more lines of YAML, the **frontmatter**.
3. A line which, after trailing whitespace is removed, consists of exactly `---`
   or `...`: the **closing delimiter**. The *first* such line ends the
   frontmatter; later ones are body text.
4. The **body**, which is arbitrary text and is never interpreted.

A file that does not begin with an opening delimiter, or that has no closing
delimiter, is not an item and **must** be rejected.

Lines are separated by either LF or CRLF. A reader **must** accept both. A
writer **must** reproduce whichever the file used, and **should** use LF for a
file it creates.

### Example

```markdown
---
id: 1
title: Support OAuth login
type: feature
status: doing
milestone: v0.1
labels:
  - auth
depends_on:
  - 3
created: 2026-09-04
updated: 2026-09-11
priority: p0
---

## Problem

Password auth is the only option, and enterprise users keep asking for SSO.
```

## 4. Keys

The frontmatter is a YAML mapping. All keys are optional except where noted.

| Key | Type | Notes |
| --- | --- | --- |
| `id` | unsigned integer | Unique within a project. Required, except that a reader **may** take it from a leading run of digits in the filename when absent. |
| `title` | string | Required in practice; an item without one is invalid. |
| `type` | string | Names a type the project declares. |
| `status` | string | Names a status the project declares. Required. |
| `milestone` | string | Names a milestone the project declares. |
| `assignee` | string | |
| `labels` | sequence of strings | A reader **must** also accept a single string, split on commas with surrounding whitespace discarded. |
| `depends_on` | sequence of unsigned integers | A reader **must** also accept a single comma-separated string, and **must** accept each element with an optional leading `#`. |
| `created` | date | `YYYY-MM-DD`. |
| `updated` | date | `YYYY-MM-DD`. |
| `source` | string | Where an imported item came from, conventionally `system:locator`, e.g. `github:owner/repo#12`. Used to make repeated imports idempotent. |

Any other key is a **custom field**. Its value **may** be any YAML scalar or
sequence. A project declares the custom fields it expects; an undeclared key is
a warning and **must not** be an error.

### 4.1 Unknown keys

> **A reader must preserve keys it does not recognise.**

This is the rule that makes version skew survivable. Without it, opening a
project with an older reader silently deletes whatever a newer one wrote. It is
not a nicety; it is the reason the format can change at all.

## 5. Ordering and layout

A writer **should** emit keys in the order given in §4, followed by custom
fields in the order they were read, so that rewriting an unchanged item produces
an identical file and a changed one produces a minimal diff.

Items **should** be named `<id>-<slug>.md`, where the slug is the title reduced
to lowercase alphanumerics separated by single hyphens, shortened only as far as
the filesystem requires. Nothing **may** depend on the name: `id` is
authoritative when present.

Subdirectories **must** be searched. Entries whose names begin with `.` or `_`,
and `README.md`, are **not** items.

## 6. YAML scalars

YAML resolves unquoted scalars before any of this applies. A hand-written
`0x1F` is a number and will be read as `31`; `12:30` and `1.20` are likewise
subject to YAML's own rules.

A writer **must** quote any value it emits that would otherwise change meaning
when read back. This confines the problem to files written by hand or by other
tools, where the remedy is to quote such values.

## 7. The project

A project directory contains a configuration file declaring the vocabulary its
items use — the types, statuses, custom fields and milestones — and a directory
of items.

The configuration format is **not** part of this specification, and a reader of
items does not require it. One property of it is:

A status belongs to exactly one **category**: `open`, `active`, `done`, or
`dropped`.

Status *names* are chosen per project and carry no meaning across a boundary:
one project's `shipped` is another's `done` is another's `closed`. Categories
are fixed, and are therefore what a consumer reasons about — whether an item is
finished, whether it is in progress, whether it was abandoned. Anything moving
items between projects **should** map by category.

## 8. Versioning and compatibility

A project records the format version it uses. Version 1 is described by this
document.

1. A minor revision **may** add optional keys, and **must not** do anything else.
2. Removing a key, changing what a key means, or making an optional key required
   requires a new version number and a migration path.
3. A reader **must** preserve keys it does not recognise (§4.1).
4. A reader encountering a version it does not understand **must** refuse the
   project and say so, naming both versions. It **must not** read it on a
   best-effort basis: misreading data is worse than declining to read it.

## 9. Conformance

An implementation conforms if it satisfies §3 through §8.

The reference implementation keeps a corpus of item files with the values they
must parse to, at [`tests/golden`][golden]. It deliberately contains files a
writer would not produce — bare strings where a sequence belongs, a missing id,
CRLF endings, keys from a version that does not exist — because those are what
people, editors and other tools produce. It is a reasonable conformance suite
for another implementation.

[golden]: https://github.com/oddurs/cairn/tree/main/tests/golden

---

Copyright © 2026 Oddur Sigurdsson. Copying and distribution of this
specification, with or without modification, are permitted in any medium without
royalty provided this notice is preserved.

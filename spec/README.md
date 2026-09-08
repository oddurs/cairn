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
4. The **body**, which is arbitrary text. This specification ascribes no
   meaning to it, and a reader is **not** required to interpret any part of it.
   A project **may** derive meaning from its content — cairn reads Markdown task
   list items as acceptance criteria — and any such reading is a convention of
   the project rather than a property of the format. Two readers that disagree
   about what a body means are both conforming.

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
| `id` | unsigned integer | Unique within a project. Required, except that a reader **may** take it from the filename when absent — see §4.1. |
| `key` | string | A short handle, unique among items of the same `type`. Optional. See §4.3. |
| `title` | string | Required in practice; an item without one is invalid. |
| `type` | string | Names a type the project declares. |
| `status` | string | Names a status the project declares. Required. |
| `milestone` | string | Names a milestone the project declares. |
| `assignee` | string | Who is doing the work. |
| `claimed` | date | `YYYY-MM-DD`. When the current claim on this item was taken. Distinct from `updated`, which any change moves. A reader **must not** infer that a claim is abandoned from this alone: how long is too long is a property of the project, not of the format. |
| `owner` | string | Who is answerable for the work, which need not be who is doing it. With people the two are usually the same; with a program working and a person answerable they are not. |
| `created_by` | string | What made the item, when that was not a person. A reader **must not** infer anything from its absence: most items have no such record. |
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

### 4.2 Identifiers, and how they are displayed

`id` is an unsigned integer. That is the whole of what it is, and it does not
change.

A project **may** display identifiers with a prefix, a suffix, or padding —
`MP-1002`, `A24`, `0001`. Such a rendering is a property of the **project**, not
of the item: it lives in the project's configuration, every item in the project
is displayed the same way, and the value stored under `id` is the integer
regardless. A reader that ignores the rendering entirely is a conforming reader.

Two consequences follow for a reader.

An identifier printed by a tool, or written in a filename, **may** therefore not
be a bare number. A reader that accepts identifiers as input **should** accept
both the rendered form and the bare integer.

The fallback of taking `id` from the filename applies to **a leading run of
digits**, and therefore only to projects whose rendering begins with the number.
A reader **may** additionally recover the identifier by applying the project's
configured rendering to the filename. A reader that cannot determine the
identifier **must** report the file as invalid rather than guess: an item whose
identity is unknown is worse than an item that fails to load.

### 4.3 Keys, and fields that name items

A project **may** declare that a field's values name other items rather than
describing this one — a milestone, a component, a parent. Such a field is a
**reference**.

A reference names its target either by `id` or by `key`. `key` exists so that a
reference stays readable in a file: `milestone: v0.1` rather than
`milestone: 42`.

A key is **not** an identifier. `id` is unique across a project, is what
`depends_on` and every id-addressed reference point at, and does not change. A
key is a handle: unique only among items sharing a `type`, chosen rather than
allocated, and permitted to change — in which case whatever changes it is
responsible for the references that named it.

Two rules make a key unambiguous, and a reader **must** apply both when
validating:

- A key **must not** be a value that the project's identifier rendering would
  produce (§4.2). Otherwise `milestone: 0042` could name either a key or a
  number depending on what happens to exist.
- A key-addressed reference resolves **only** by key. A reader **must not** fall
  back to matching an identifier or a title.

Which fields are references, what they target, and whether they are addressed by
id or by key are declared in the project's configuration, which §7 places
outside this specification. A reader that does not consult the configuration
**should** treat such values as opaque strings, which is what they are.

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

The frontmatter is YAML, and YAML resolves unquoted scalars before any of this
applies. A reader **must** resolve them according to the **YAML 1.2 core
schema**.

Naming the version is not pedantry. YAML 1.1 and 1.2 disagree about exactly the
values people write by hand, and a reader using the wrong one will report
different content for the same file:

| Written | YAML 1.2 core (required) | YAML 1.1 (wrong here) |
| --- | --- | --- |
| `no` | the string `no` | the boolean false |
| `12:30` | the string `12:30` | the integer 750, read as sexagesimal |
| `0x1F` | the integer 31 | the integer 31 |
| `1.20` | the float 1.2 | the float 1.2 |

Under 1.2 core, only `true` and `false` (and their capitalised and upper-case
spellings) are booleans; `yes`, `no`, `on` and `off` are strings.

This is worth checking rather than assuming: several widely used YAML libraries
still implement 1.1 by default, including PyYAML. The reference reader in
`reader.py` adjusts for it, and says where.

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

That category is the only thing in a project's configuration a reader **must**
honour. A configuration will describe much else — how a status is coloured, what
it is called in a listing, which columns a table shows, how a roadmap is laid
out. None of it changes what an item *is*, and a reader that ignores all of it
conforms, in the same way as §4.3.

Where a configuration expresses something as an ordered sequence — the statuses,
the permitted values of a field — that order is **significant**, and a reader
that presents them **should** present them in it. A tool rewriting a
configuration **must not** reorder such a sequence.

## 8. Versioning and compatibility

A project records the format version it uses. Versions 1 and 2 are both
described by this document: no key in it changed meaning between them.

The version covers the on-disk shape of a **project**, not only of an item. A
project is its configuration and its items (§7), so a change to the shape of the
configuration alone requires a new version and a migration even when no key in
this document changes meaning.

The reason is §4.1, and its absence. An item reader **must** preserve keys it
does not recognise, so an item may gain an optional key without a new version.
No such rule is stated for the configuration, and none is to be assumed: a
configuration format is free to refuse what it does not know, and one that does
cannot gain a key without every existing reader refusing the project. So a
project **must** take a new version number when its configuration gains a key,
unless the configuration format it uses states that unknown keys are ignored.

A reader of items alone is unaffected by such a change, and **should** say so
rather than refusing a project it can in fact read.

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

Beside it is one directory per format that has ever existed, holding items as
that format wrote them with the values they parsed to then. An implementation
that intends to read what people have already written should run against those
too, not only against the current corpus.

## 10. Conventions (non-normative)

Nothing in this section is required of a conforming reader. It is written down
because §3 says the body may be given meaning by a project, and a second
implementer is better served by knowing what the reference implementation
actually does than by discovering it.

### 10.1 Acceptance criteria

cairn reads Markdown task list items in the body as **acceptance criteria**.

A line is a criterion when, after leading whitespace is removed, it begins with
a list marker (`-`, `*` or `+`) followed by a space, then `[ ]`, `[x]` or `[X]`,
and then **something else**. `[x]` and `[X]` are ticked; `[ ]` is not.
Indentation is allowed, so nested criteria count.

A box with nothing after it is a **placeholder, not a criterion**. The templates
cairn ships end with a bare `- [ ]` prompting the author to write one, and
counting it would leave every new item permanently short of its own criteria.

A project may confine the count to one section, named in its configuration; the
count then covers the lines under a heading with that name, at any level and
matched without regard to case. With no section named, the whole body counts.

This is a reading of arbitrary text, and §3 means what it says: a reader that
ignores it entirely conforms.

---

Copyright © 2026 Oddur Sigurdsson. Copying and distribution of this
specification, with or without modification, are permitted in any medium without
royalty provided this notice is preserved.

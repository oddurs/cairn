---
id: 90
title: The body may be interpreted, and the specification should say so
type: bug
status: done
milestone: v0.2
created: 2026-09-07
updated: 2026-09-07
priority: p1
effort: s
sprint: s9
---

## Problem

§3 of the specification says, of an item's body:

> The **body**, which is arbitrary text and **is never interpreted**.

That was true when it was written and is not true now. `0074` reads
`- [ ]` and `- [x]` out of the body to count acceptance criteria, and `cairn
close`, `cairn show`, the filter language and the Model Context Protocol server
all report what it finds.

So a normative document says the opposite of what the reference implementation
does. That is worse than either being wrong on its own, and by this project's
own rule the specification should have changed first. I did not, and the rule
caught nothing because nothing checks prose against behaviour.

## Proposal

§3 says what is actually true, which is a more useful statement anyway:

> The **body** is arbitrary text. This specification ascribes no meaning to it,
> and a reader is not required to interpret any part of it. A project **may**
> derive meaning from its content — cairn counts Markdown task list items as
> acceptance criteria — and any such reading is a convention of the project
> rather than a property of the format. Two readers that disagree about the
> body's meaning are both conforming.

The distinction being drawn is between *the format ascribes no meaning* — still
true, and the important guarantee — and *nobody may ever look at it*, which was
never what was meant and is now plainly false.

## The larger gap

This was found by reading, which is the least reliable way to find anything.
Nothing tests the specification against the program.

`spec/reader.py` gets close: it is written from the document and disagreeing
with cairn is a failure. But it only covers §3 and §4, the parts about parsing.
A claim in prose like "never interpreted" has nothing checking it at all.

Worth a note in the item that closes this: the conformance run is the right home
for such a check if anybody thinks of a way to write one.

## Acceptance criteria

- [x] §3 says what is true, and keeps the guarantee that matters
- [x] It says explicitly that a project's reading is a convention, not a format
      property
- [x] The version number does not move — this is a clarification, and calling it
      anything else would cheapen the number
- [x] The acceptance-criteria convention is described where a second implementer
      would find it

## 2026-09-07

Done. Spec §3 now says the body may be given meaning by a project and that any such reading is a convention rather than a property of the format, so two readers that disagree are both conforming. A non-normative §10 describes how cairn actually reads acceptance criteria, including that a box with nothing after it is a placeholder. The version number does not move.

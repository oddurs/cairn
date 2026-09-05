---
id: 26
title: Specify the item file format
type: docs
status: done
milestone: v1.0
created: 2026-09-04
updated: 2026-09-04
priority: p0
sprint: s4
effort: m
---

## Problem

The format is described by the implementation. Nobody can write a compatible
tool, and nobody can be sure their five-year-old items will still open.

## Proposal

A normative chapter in the manual: every frontmatter key, its type, whether it
is required, what a reader must do with keys it does not recognise, and how the
body is delimited. Written so someone could implement a reader from it alone.

## Acceptance criteria

- [ ] Every key specified, including the ones cairn only writes
- [ ] Unknown-key behaviour defined (preserve, never drop)
- [ ] A second implementation could be written from the chapter alone

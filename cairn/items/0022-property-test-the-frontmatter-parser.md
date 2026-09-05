---
id: 22
title: Property-test the frontmatter parser
type: chore
status: done
milestone: v0.2
created: 2026-09-04
updated: 2026-09-04
priority: p1
sprint: s2
effort: m
---

## Problem

The parser is the trust boundary for three kinds of input: what people type,
what agents write, and what import brings in from elsewhere. It is covered by
example tests only.

## Proposal

Property tests over parse -> render -> parse: any item that parses must
re-render to something that parses identically. Generate adversarial titles and
bodies — Unicode, embedded `---`, very long lines, empty fields, CRLF.

## Acceptance criteria

- [ ] Round-trip fidelity property, several thousand cases per run
- [ ] Parser never panics on arbitrary bytes
- [ ] Corpus of failures found is kept as regression tests

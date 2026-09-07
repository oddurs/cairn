---
id: 69
title: Test the way that has actually been finding things
type: chore
status: done
milestone: v0.2
created: 2026-09-06
updated: 2026-09-06
priority: p1
sprint: s7
effort: m
---

## Problem

There are three thousand lines of test code, and roughly none of the eleven
defects found so far came from the example tests among them. They came from a
soak test, from a platform nobody here can run, from an adversarial reader, and
from attempting a release.

The lesson is not that example tests are worthless — they stop regressions, and
they have. It is that they only ever check what somebody already thought of, and
this project has been finding its bugs in the space nobody thought of.

Two hundred lines of soak test found three defects in an afternoon. That is the
best return of anything written here.

## Proposal

Spend where the returns have been.

- **Soak wider.** It exercises the ordinary commands. It has never touched
  `import`, `export`, `renumber --compact`, `milestone`, or the MCP server, and
  it never interleaves two processes.
- **Property-test past the parser.** The filter grammar, the identifier
  rendering from `0062`, and the interchange round trip are all total functions
  with properties worth stating.
- **Fuzz the command line.** Every argument surface takes text from a person or
  a model. A crash there is a bug wherever it is.
- **Keep the example tests.** They are the regression net, and every defect
  found by the other methods should leave one behind.

## Acceptance criteria

- [ ] The soak covers every mutating command, and a concurrent mode
- [ ] Properties stated for the filter grammar and the interchange round trip
- [ ] Arbitrary argument input never panics
- [ ] Every defect found this way leaves an example test behind

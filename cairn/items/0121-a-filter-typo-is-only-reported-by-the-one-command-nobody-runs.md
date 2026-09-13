---
id: 121
title: A filter typo is only reported by the one command nobody runs
type: bug
status: done
milestone: v0.1
created: 2026-09-13
updated: 2026-09-13
priority: p1
area: filter
---

## What happens

`cairn check` reports a view filter naming a field the schema does not declare.
No other command does, and it is the same typo.

```
cairn check
  cairn.toml:234: view `broken` filter names `nonsense`, which is not a
  declared field — items may still carry it, but nothing in this schema
  says they do

cairn list --view broken      # "no items match", exit 0, no warning
cairn list -f "nonsense=x"    # "no items match", exit 0, no warning
```

The wording and the line number in `check` are right, and a warning rather than
an error is the right call: items may legitimately carry undeclared fields. The
gap is that the knowledge is confined to `check`. Somebody who never runs it
gets `no items match` — a true statement about a filter that does not mean what
they typed.

## What should happen

Every command that takes an expression — typed at `-f` or loaded from a
`[[view]]` — says the same thing, in the same words, on stderr. Exit status
unchanged, results still shown, `--json` and `--plain` still machine-readable.

## Notes

Surfaced by harrow, which reads cairn's saved views and implements a narrower
filter grammar, so a view that is valid cairn silently shows empty there. That
is harrow's bug, but it asked a question that was cairn's to answer: is the
filter grammar public interface or an implementation detail?

It is already answered, in the Stability chapter, which promises "the filter
grammar — field names, operators and the pseudo-fields `category`, `blocked`,
`ready` and `blockers`. Additive only." And §7 of the specification says the
configuration format is not part of it and a reader ignoring it conforms.

So: a promise about the *program*, not about the *format*. What was missing was
that nothing enforced it — it was the one promise in that chapter with no test —
and nobody writing a view was told which dialect they were writing in.

## Acceptance criteria

- [x] `list`, `board`, `next`, `search`, `export` and `set` warn, in check's words
- [x] The warning is on stderr; `--json`, `--ids`, `--count` and `--plain` stay clean
- [x] Exit status is unchanged and results are still shown
- [x] `render` warns for a person and stays silent under `-q`, which the hook passes
- [x] A declared field, a built-in and a derived key are never called a typo
- [x] One list of known keys, shared with `check`, so the two cannot drift
- [x] `tests/stability.rs` pins the grammar the Stability chapter promises
- [x] The manual says the grammar is a promise about the program, not the format
- [x] The generated `[[view]]` block says which dialect its filter is in

## 2026-09-13

Answered rather than decided: the Stability chapter already promises the grammar, and spec §7 already excludes the configuration format. The work was making that enforceable and findable — a stability test, a line in the Filters chapter, a line in the generated cairn.toml, and a paragraph in Integrating saying a second program should run `cairn list --view NAME --json` rather than reimplement the grammar. The stability test found four derived keys documented only as `@item` entries, and now accepts both texinfo spellings.

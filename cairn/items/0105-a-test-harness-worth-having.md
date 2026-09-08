---
id: 105
title: A test harness worth having
type: chore
status: done
milestone: v0.1
created: 2026-09-08
updated: 2026-09-08
priority: p1
sprint: s10
effort: l
area: testing
---

## Problem

Six test files, and between them **four `Project`s, three random generators,
three `path_with_binary`s and two `Out`s**. They had drifted: `run` returned
`Out` in one file, `(i32, String, String)` in another and `(i32, String)` in a
third; `expect` returned `Out` in two and `String` in three.

That is not untidiness, it is a tax. Every improvement to the harness has to be
made four times or not at all — which is how they diverged in the first place,
each one copied and then edited where it landed.

The second problem is worse because it costs time on every test written.
Configuring a project means string surgery on the shipped template:

```rust
let cfg = p.read("cairn.toml")
    .replace("[render]", "[render]\nheader = \"docs/intro.md\"")
    .replace("link_items = false", "link_items = true")
    .replace("group_by = \"milestone\"", "group_by = \"epic\"");
```

Forty-one of those. They break constantly and always for the same reasons: a
`[render]` table appended twice because the template already had one, a
`link_items = false` that had moved, a `title = "Roadmap"` that was not where
the test guessed. Every failure is about the *file*, not about cairn.

And the assertions do not help. `assert_contains` prints the whole haystack, so
a mismatch inside a five-kilobyte JSON document prints five kilobytes.

## Proposal

**One harness**, in `tests/support/`, that every integration test shares.

**A schema built from values rather than edited as text.**

```rust
Project::with(
    Schema::standard()
        .field(Field::text("risk").agent(Agent::ReadOnly))
        .amend_status("done", |s| s.agent(Agent::Propose))
        .render(|r| r.group_by("epic").link_items()),
)
```

A schema assembled from parts cannot be wrong about the file it is editing,
because it is not editing one. It should refuse a duplicate field at the line
that added it, and `Schema::standard()` should itself be checked against the
real program — a builder that produces something cairn rejects is worse than no
builder, because every test using it fails for a reason none of them is about.

**Assertions that print the difference**, not the haystack.

**A coverage floor**, so the number a sprint moved cannot quietly rot.

## Acceptance criteria

- [x] One `Project`, one generator, one `Out`, shared by every test file
- [x] The generator keeps the same sequence, so recorded seeds still mean what they meant
- [x] A schema builder covering statuses, types, fields, refs, views, hooks and rendering
- [x] The builder refuses a duplicate at the point of the mistake
- [x] The builder is tested against cairn itself
- [x] Assertions show an excerpt and the nearest lines
- [x] `make coverage` with a floor, in continuous integration
- [x] The harness is documented where somebody about to write a test will read it

## 2026-09-08

Done. tests/support/ is 1,400 lines shared by all six integration files; between them they lost about 500 lines of duplicated harness. The generator is splitmix64, which is what all four copies used, so every seed recorded in CI or a failure report still names the same run. The builder covers statuses (including one that declares no category), types, every field kind including refs by key and by id, views, hooks and the render block; it refuses duplicates with the offending line as the caller; and six tests in the harness check it against the real program. `make coverage` has a floor of 90% of regions, currently 91.38%, and runs as its own CI job.

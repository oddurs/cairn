---
id: 62
title: One template for how identifiers are written and read
type: feature
status: backlog
milestone: v0.2
created: 2026-09-06
updated: 2026-09-06
priority: p0
sprint: s6
effort: m
---

## Problem

Identifiers are `0001`, and that is all they can be. Teams want `MP-1002`,
`A24`, `CAIRN-TOOLS-24` — a project key, so an identifier can be said aloud in a
meeting and pasted into a commit message without ambiguity about which project
it belongs to.

## The design question

Is the key part of the identifier, or part of how the identifier is written?

Making it part of the identifier — `id: MP-1002` in the frontmatter — is the
obvious reading of the request and the wrong answer. The specification commits
to `id` being an unsigned integer, so changing it is a new format version and a
migration for every existing project. It would also break the machinery that
depends on identifiers being numbers: allocation is `max + 1`, `renumber`
compares and reassigns, `depends_on` is a list of integers, and import maps
identifiers across projects.

So the key is a **rendering**. `id` stays an integer; the project declares how
that integer is written and read back.

That is not a compromise forced by the format — it is the better design, and the
format promise is what made it obvious. A project renaming its key changes how
things are displayed, not what anything *is*, and nothing that refers to an item
by number breaks.

## Proposal

One setting rather than three:

```toml
[project]
id_format = "{n:04}"           # 0001         — the default
id_format = "MP-{n}"           # MP-1002
id_format = "A{n}"             # A24
id_format = "CAIRN-TOOLS-{n}"  # CAIRN-TOOLS-24
```

`{n}` is the number, `{n:04}` pads it. The syntax is the one the audience
already knows from every format string they use.

The same template parses input, so `cairn show MP-1002` works — and so does
`cairn show 1002`, because having to type a prefix you already know is friction
for nothing. Matching the prefix is case-insensitive.

Three settings — key, separator, padding — would express the same thing less
clearly and would allow combinations nobody wants. One template is
self-documenting: you can see what it produces.

## Acceptance criteria

- [ ] All three examples in the title are expressible
- [ ] `id` in the frontmatter is unchanged: still an integer, no format bump
- [ ] Both the rendered form and the bare number are accepted as arguments
- [ ] Prefix matching is case-insensitive
- [ ] A malformed template is rejected at load, naming the problem
- [ ] `ref` in JSON output carries the rendered form; `id` stays numeric
- [ ] The default is exactly what projects get today

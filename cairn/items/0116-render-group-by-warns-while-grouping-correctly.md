---
id: 116
title: render.group_by warns while grouping correctly
type: bug
status: done
milestone: v0.1
created: 2026-09-12
updated: 2026-09-12
priority: p1
area: check
---

## What happens

`cairn check` warns on every run that `render.group_by` has nothing to group
by, while the roadmap it renders from the same schema groups correctly.

```
cairn: cairn.toml:214: render.group_by is `release`, but no [[type]] declares
`groups`, so the roadmap has nothing to group by
```

The check asks whether some `[[type]]` declares `groups`. That is not the
question. The question is whether `render.group_by` names something items can
carry — a declared field, a grouping type, or the built-in `milestone` key.
Grouping by a plain `[[field]]` has always worked, so the warning is wrong for
every project that does it.

## What should happen

Warn only when grouping by that name would genuinely empty the roadmap: the
name is undeclared *and* the schema declares some other type that groups, which
is the renamed-field mistake the warning was written for.

## Reproduction

1. `cairn init --bare`, remove the `[[type]] milestone` block
2. Declare `[[field]] name = "release"`, set `render.group_by = "release"`
3. `cairn check` warns; `cairn render` groups anyway

## Acceptance criteria

- [x] Grouping by a declared field does not warn
- [x] Grouping by the built-in `milestone` key does not warn
- [x] Grouping by a name nothing declares, where another type groups, warns and names the type
- [x] `cairn board --group-by` can group by any name items carry

## 2026-09-12

Fixed. The condition in `src/cmd/check.rs` asked whether any type declares `groups`; it now asks whether `render.group_by` resolves to something items carry, and warns only in the two arrangements where grouping really does collapse — naming the type to use, or the missing `groups`. `board::column_values` also groups by a `milestone` kept as a plain label.

---
id: 115
title: A grouping type's items are unusable without a key
type: bug
status: done
milestone: v0.1
created: 2026-09-12
updated: 2026-09-12
priority: p0
area: refs
---

## What happens

A pristine `cairn init` models releases as items: `[[type]] milestone` with
`groups = "one"`. The comment beside it says declaring it also creates the
field, so `milestone: v0.1` names one by its key.

Nothing can be filed under a milestone, because nothing ever gives the
milestone a key.

```sh
cairn new "v1.0" -t milestone -q     # 0001
cairn new "some work" -q             # 0002
cairn set 2 milestone=0001
cairn: `milestone` names `0001`, which does not exist
nothing to name yet
```

## What should happen

Creating an item of a grouping type should give it a key, derived from its
title, so the field the type implies has something to name. The diagnostic when
a reference misses should say which of the two things is wrong: no such item,
or an item with no key.

## Reproduction

1. `cairn init --bare`
2. `cairn new "v1.0" -t milestone`
3. `cairn new "some work"`
4. `cairn set 2 milestone=v1.0`

## Acceptance criteria

- [x] `cairn new -t <grouping type>` assigns a key from the title
- [x] An explicit `--set key=` still wins
- [x] A title that cannot make a key is refused with a message that says so
- [x] `nothing to name yet` is not printed when items of the type exist
- [x] `cairn check` reports a grouping-type item that has no key
- [x] `cairn config` lists the field a grouping type implies
- [x] The reproduction above works end to end, including `cairn roadmap`

## 2026-09-12

Fixed. `cairn new` derives the key from the title in `src/cmd/new.rs`, refusing the two titles that cannot make one rather than leaving a keyless container behind. `refs::unresolved` now tells the two failures apart, `check` reports a keyless grouping item, and `cairn config` lists implied fields and marks a milestone with no key.

## 2026-09-12

The Configuration chapter still documented `[[milestone]]` as a live block with a `cairn milestone` command to edit it; neither has existed since format 3, and it is the first thing somebody hitting this bug would have read. Rewritten to describe the grouping type and the derived key.

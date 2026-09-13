---
id: 119
title: Lowercasing could put a combining mark in a filename
type: bug
status: done
milestone: v0.1
created: 2026-09-12
updated: 2026-09-12
priority: p2
area: item
---

## What happens

`slug` tested a character for `is_alphanumeric` and then pushed whatever
`to_lowercase` produced. Those are not the same set: Turkish dotted capital I is
a letter, and lowercases to `i` followed by U+0307 COMBINING DOT ABOVE, which is
not.

So `cairn new "İstanbul rewrite"` wrote a filename carrying an invisible
combining mark — one more thing for a filesystem to normalise however it likes,
and unmatchable by anyone typing the name.

## What should happen

Filter after lowercasing rather than before, so the guarantee the property test
already asserts — every character alphanumeric or a dash — actually holds.

## Reproduction

1. `cairn new "İstanbul rewrite"`
2. The filename contains U+0307.

## Notes

Predates 0.2.0. Found by `slugs_are_always_usable_as_filenames`, which has
asserted the right thing all along and had simply never generated that input;
proptest reached it while this branch was being tested. The counterexample is
pinned twice now: as a unit test, and in `proptest-regressions/item.txt`.

`key_from_title` was written with the same shape and had the same hole, so both
are fixed together.

## Acceptance criteria

- [x] `slug("İ")` is `i`
- [x] `key_from_title("İstanbul")` is `istanbul`
- [x] The property test passes at 8000 cases
- [x] The counterexample is pinned so seed choice cannot hide it again

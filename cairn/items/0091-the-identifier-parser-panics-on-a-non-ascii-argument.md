---
id: 91
title: The identifier parser panics on a non-ASCII argument
type: bug
status: backlog
milestone: v0.1
created: 2026-09-07
updated: 2026-09-07
priority: p0
sprint: s9
effort: s
area: config
---

## Problem

`IdFormat::read` and `id_in_filename` strip the project's identifier prefix
with `strip_prefix_ci`, which guards on byte length and then slices on a byte
index:

```rust
if s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix)
```

A guard in bytes and a slice in bytes is correct arithmetic and the wrong
question. If the byte at that index is in the middle of a character, the slice
panics.

```
$ # id_format = "MP{n}"
$ cairn show 'aé'
thread 'main' panicked at src/config.rs:592:36:
end byte index 2 is not a char boundary; it is inside 'é' (bytes 1..3)
```

Any project with a non-empty prefix, any argument that is not ASCII. The prefix
does not have to be exotic; `MP` is two bytes and that is enough.

`strip_suffix_ci` has the same shape. And `id_in_filename` calls the same
function against names read off the disk, so this is not only reachable from
something somebody typed: a file in the items directory whose name happens to
begin with a multi-byte character is enough, which makes it a crash in `list`
rather than in an argument parser.

## Proposal

Ask for a prefix-length slice and accept that there may not be one:
`s.get(..prefix.len())`, or `split_at_checked`. A non-boundary index then
answers "this is not that prefix", which is the truth.

The regression test belongs with the identifier tests and should exercise both
paths — an argument and a filename — because the second is the one nobody would
think to try.

## Acceptance criteria

- [ ] `cairn show` with a non-ASCII argument reports an error rather than panicking
- [ ] A non-ASCII filename in the items directory does not crash `cairn list`
- [ ] Both `strip_prefix_ci` and `strip_suffix_ci` are fixed, not only the one that was reported
- [ ] A test covers the argument path and the filename path

---
id: c0d4a6c8-d5f5-4898-83e8-e54b29d57a81
title: A Windows checkout reports the roadmap stale for ever
type: bug
status: done
milestone: v0.1
created: 2026-09-19
updated: 2026-09-19
closed_at: 2026-09-19
priority: p1
area: render
effort: s
---

## What happens

`cairn render --check` and `cairn check --render` compare the generated roadmap
with the file on disk as bytes:

```rust
let current = std::fs::read_to_string(&target).unwrap_or_default();
if current == markdown {
```

`markdown` is generated with LF throughout. `ROADMAP.md` carries no `-text`
attribute in `.gitattributes`, so a checkout with `core.autocrlf` — the default on
Windows, and what the GitHub runners use — converts it to CRLF on the way out of
git.

The two can then never be equal. A Windows user is told the roadmap is out of
date on every run, and `cairn render` rewrites it as LF, which git immediately
reports as modified. There is no state in which the project is quiet.

Found by a test that cloned a configured project and ran `cairn check --render
--strict` in the clone: green on Linux and macOS, red on Windows with

```
cairn: ROADMAP.md: out of date — run `cairn render`
```

cairn's own CI does not catch it because the `roadmap` job runs only on Linux.

## What should happen

cairn already solved this for item files, and the answer should be the same one:
`Eol::detect` reads whichever ending a file uses, and `eol.apply` writes the same
back, so a checkout with `core.autocrlf` set never turns an edit into a whole-file
diff. The generated roadmap deserves the same treatment.

Two parts:

- Compare with line endings normalised, so a CRLF checkout is not called stale for
  a difference that is not a difference.
- Write the file back with the ending it already had, so rendering a CRLF file
  does not convert it and produce a diff of every line.

An alternative is `ROADMAP.md -text` in the generated `.gitattributes`, which
stops git converting it. Cheaper, and worse: it only helps projects that adopt the
new `cairn init` output, does nothing for the ones that already exist, and leaves
the comparison as brittle as it was.

## Reproduction

1. On Windows, or with `git config core.autocrlf true`, clone a project with a
   committed `ROADMAP.md`
2. `cairn check --render` — reports it stale
3. `cairn render` — rewrites it; `git status` now shows every line changed

## Acceptance criteria

- [x] A CRLF checkout is not reported stale when the content matches
- [x] Rendering a CRLF file keeps CRLF rather than converting it
- [x] An LF file stays LF, on every platform
- [x] A test covers both endings, and runs on Windows in CI

## 2026-09-19

Fixed. `render::matches_on_disk` compares with endings normalised and `render::as_written` writes back with the ending the file already had — the same pair `Eol::detect`/`eol.apply` gives item files, applied to the one generated file. Both callers, `check --render` and `render --check`, go through the one comparison so they cannot drift.

Took the normalising fix rather than `ROADMAP.md -text` in the generated `.gitattributes`: that would only help projects created after the change, does nothing for the ones that already exist, and leaves the comparison as brittle as it was.

Verified all four states: a CRLF file matching is not stale; rendering it keeps CRLF; a genuine change is still reported on a CRLF file; a fresh file is LF. The test constructs both endings rather than relying on the platform, because this was found by a failure that only happened on Windows.

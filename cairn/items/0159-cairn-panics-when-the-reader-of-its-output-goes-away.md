---
id: 159
uid: 1ca8d4eb-a24e-45f3-9c65-27ac7d45e501
title: cairn panics when the reader of its output goes away
type: bug
status: done
assignee: oddurs
created: 2026-09-27
updated: 2026-09-28
closed_at: 2026-09-28
priority: p2
area: cli
effort: s
---

## What happens

A reader that closes the pipe early makes cairn panic instead of exiting
quietly:

```
$ cairn claim 0155 | head -1
claimed 0155  A finished item records a Result that dependents can quote

thread 'main' panicked at library/std/src/io/stdio.rs:1166:9:
failed printing to stdout: Broken pipe (os error 32)
```

Seen 2026-09-27 while working milestone 0153. `println!` panics on EPIPE;
Unix tools exit silently (or with SIGPIPE's status) when their reader goes
away. The write the command was making still happened, so this is noise,
but a script piping `cairn list` into `head` sees a panic and a non-zero exit.

## Approach

Exit quietly on a broken pipe: restore the default SIGPIPE disposition at
startup on Unix, or route output through a writer that treats EPIPE as the end.

## Acceptance criteria

- [x] `cairn list | head -1` prints one line and no panic
- [x] A test pipes a command's output into a reader that closes early and asserts no panic on stderr

## 2026-09-28

Fixed without unsafe and without a new dependency: resetting SIGPIPE would need libc or an FFI declaration, and this codebase has no unsafe at all. main runs the command under catch_unwind; a panic hook stays quiet for std's 'failed printing to stdout/stderr' broken-pipe panics, which then unwind (so a command printing while it holds the project lock releases it) and exit 0; every other panic is reported and re-raised as before. tests/pipes.rs closes the read end before cairn writes: both tests failed with the panic before the fix. By hand: cairn list -A | head -1 three times, one line, no panic.

## Result

A reader that goes away (cairn list | head) ends cairn quietly with status 141, as SIGPIPE would: broken-pipe panics are caught in main, unwound so the project lock is released, and not reported; the command may have stopped partway, and the status says so. Any other panic is unchanged.

## 2026-09-28

Review (/code-review medium) found that exit 0 was wrong: a command can stop partway at a print, e.g. a bulk set after its first item or migrate after its first line, and a truncated check under set -o pipefail would pass. Now exit 141, what SIGPIPE gives, still without a panic trace; and the hook is installed before --help and --bug-report can print. Declined making every print tolerate EPIPE so work continues after the reader leaves: that is a larger change across every command, and 141 already tells a script the output was cut short.

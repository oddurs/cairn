---
id: 159
uid: 1ca8d4eb-a24e-45f3-9c65-27ac7d45e501
title: cairn panics when the reader of its output goes away
type: bug
status: backlog
created: 2026-09-27
updated: 2026-09-27
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

- [ ] `cairn list | head -1` prints one line and no panic
- [ ] A test pipes a command's output into a reader that closes early and asserts no panic on stderr

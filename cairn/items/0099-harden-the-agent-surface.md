---
id: 99
title: Harden the agent surface
type: chore
status: done
milestone: v0.1
created: 2026-09-07
updated: 2026-09-07
priority: p0
sprint: s10
effort: l
area: mcp
---

## Problem

`mcp.rs` is 845 lines, the second-largest file in the project, and carries nine
tests. Coverage per line is the thinnest anywhere, and the exposure is the
highest, for three reasons that have nothing to do with its size.

**It is the only surface where a non-human drives writes** with nobody reading
the output. Every other write path has somebody watching what came back.

**It is the only place the permission model is enforced.** `Agent::ReadOnly` and
`Agent::Propose` are a guard rail on the command line — the doc comment says so,
because an agent with a shell can run `cairn set` — and a boundary here. Every
enforcement bug in this file is therefore a real one rather than a theoretical
one.

**Its one piece of `unsafe` is reached from remote input.**

```rust
unsafe { std::env::set_var("CAIRN_AGENT", name) };
```

`name` is `clientInfo.name`, off the wire. The call is almost certainly sound —
single-threaded, during `initialize`, before anything else runs — but it is the
only `unsafe` in the program and it deserves to be argued rather than assumed.

Per-tool coverage is uneven in a way the totals hide. Of twelve tools,
`list_items` and `search_items` appear **once each** in the entire suite;
`add_note`, `close_item`, `show_item`, `update_item`, `get_schema` and
`next_items` twice.

## Proposal

A review pass and a test matrix.

**Review.** Write down why the `set_var` is sound, in the code, or remove the
need for it by threading the client through instead. Check every tool for the
gap between the JSON schema it advertises and what it actually accepts: a tool
that documents a required argument and tolerates its absence is lying to the
model on the other end.

**Tests.** One per tool, asserting its advertised schema matches what it
accepts. Then the cases nobody writes by hand:

- Every tool against a field declared `agent = "read-only"` and one declared
  `agent = "propose"`, asserting refusal and that nothing was written
- A status declared `agent = "propose"`, moved to by `close_item`
- `tools/call` before `initialize`
- `clientInfo.name` containing a newline, a NUL byte, 10KB, and a shell
  metacharacter --- it becomes an environment variable and lands in
  `created_by`
- Two clients against one project at once
- A tool called with arguments of the wrong JSON type, not merely missing

## Acceptance criteria

- [x] Each of the twelve tools has a test asserting its schema matches what it accepts
- [x] Read-only and propose are tested on a field and on a status, for refusal *and* for nothing written
- [x] `clientInfo.name` is tested with a newline, a NUL, something very long, and a metacharacter
- [x] `tools/call` before `initialize` behaves deliberately
- [x] Two concurrent clients are tested
- [x] The `unsafe` is either justified in a comment or removed

## 2026-09-07

Done, and it found more than the item predicted. The permission model was not enforced over MCP at all: create_item and update_item applied the caller's `fields` object with `apply` rather than `apply_requested`, and claim_item and close_item did the same for status — so every custom field, every status move and every close went straight past the check. The command line enforced it and the server did not, the exact inverse of what the manual and the code comment both claimed. Also: a NUL byte in clientInfo.name reached `env::set_var`, which panics, and killed the server mid-stream. The `unsafe` is gone — identity is a process global that `acting_agent` consults, hooks are passed CAIRN_AGENT explicitly rather than by inheritance, and the name is sanitised once before it becomes an assignee. Eleven tools, not twelve; the twelfth was serverInfo.name in a grep.

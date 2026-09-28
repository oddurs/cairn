---
id: 160
uid: 36c958b9-fe94-4722-9bff-bb8cbb5ac195
title: MCP show_item still describes ids as format-4 UUIDs
type: bug
status: done
assignee: oddurs
created: 2026-09-27
updated: 2026-09-28
closed_at: 2026-09-28
priority: p3
area: integration
effort: s
---

## What happens

MCP `show_item` describes its `id` as "Full UUIDv4, unambiguous prefix (at
least 8 hex digits), or migrated legacy number". That is format 4's wording.
In format 5 an id is a number, in any rendering the project declares, or a
uid, whole or by prefix. Agents read tool descriptions; this one tells them to
send the wrong thing.

## Approach

Describe the id as every other tool does ("Item id"), and check no other tool
description still speaks of format 4.

## Acceptance criteria

- [x] No MCP tool description mentions format 4's UUID ids

## 2026-09-28

Every tool's id argument now comes from one id_prop(), so the wording cannot drift per tool again. The description names the number and the project's rendering of it (12, 0012, BUG-12), which is what agents should send; it does not advertise uids, which still resolve but are for durable references in data, not for addressing items in a conversation. tests/agents.rs no_tool_describes_format_four_ids reads tools/list and fails on 'UUID', 'legacy number' or '8 hex digits' in any tool, and requires every id description to show 0012; it failed on show_item before the fix.

## Result

MCP tools describe an item id one way, from one function: its number or the project's rendering of it (12, 0012, BUG-12). No tool mentions format 4's UUIDs; a test on tools/list keeps it so.

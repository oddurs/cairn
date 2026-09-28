---
id: 160
uid: 36c958b9-fe94-4722-9bff-bb8cbb5ac195
title: MCP show_item still describes ids as format-4 UUIDs
type: bug
status: backlog
created: 2026-09-27
updated: 2026-09-27
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

- [ ] No MCP tool description mentions format 4's UUID ids

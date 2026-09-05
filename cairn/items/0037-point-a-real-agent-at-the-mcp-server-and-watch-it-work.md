---
id: 37
title: Point a real agent at the MCP server and watch it work
type: chore
status: backlog
milestone: v0.1
created: 2026-09-05
updated: 2026-09-05
priority: p0
effort: s
---

## Problem

The thesis is that agents work the backlog through tools rather than prose. The
MCP server is verified only by protocol tests written by the same person who
wrote the server. It has never been wired into a client and driven by an agent
doing an actual task.

That is the central claim, and it is the least tested thing here.

## Proposal

Wire `cairn mcp` into a real client, give an agent a real task in a real
project, and watch. Record what it reaches for, what it gets wrong, and which
tool descriptions mislead it.

The output is not a passing test. It is a list of the places where the tools
say something other than what an agent needs to hear.

## Acceptance criteria

- [ ] An agent finds work, claims it, does it and closes it without being told
      the commands
- [ ] It never writes a TODO or plan file alongside the backlog
- [ ] Every tool description that misled it is rewritten
- [ ] Whatever it could not do through tools is filed

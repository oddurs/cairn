---
id: 136
title: Make the approved queue explicit to agents
type: feature
status: planned
milestone: v0.3
owner: oddurs
created_by: codex
created: 2026-09-23
updated: 2026-09-23
priority: p1
effort: m
area: workflow
---

## Problem

In the assessed checkout, `next` excludes closed items and unfinished dependencies. It does not distinguish an untriaged idea from approved work, nor a status named blocked from a runnable item. 0001 was in progress without a claimant although its body said it needed a maintainer credential. An autonomous `claim --next` could repeatedly pick the wrong kind of work.

The repository now scopes autonomous claims to `status=planned`. That is an explicit local convention, not a change to the command's default semantics.

## Proposal

First exercise the configured loop with real agent sessions. Prefer the existing filter/view vocabulary for declaring eligible work. Decide whether a project-selected view or filter should feed next, atomic claim-next, generated instructions, and MCP. A bare status spelling must never acquire hardcoded meaning. Explicitly address the difference between dependency readiness, workflow selection, and assignment.

## Acceptance criteria

- [ ] A fixture with renamed statuses, an untriaged idea, external waiting work, a dependency, and an approved item demonstrates the intended selection.
- [ ] CLI next, atomic claim-next, MCP, and generated instructions follow the same documented policy.
- [ ] Projects without an explicit policy retain the documented existing behaviour.
- [ ] The policy composes with a caller's narrower filter and does not silently broaden an agent's scope.
- [ ] The work includes a decision on whether configuration is necessary, backed by an observed session.

## Boundaries

No scheduler, background worker, hardcoded planned status, or new command just for this project.

## Released by codex

Ship the already-landed maintenance fixes first, as required by the v0.3 release sequence; then implement the approved-view contract.

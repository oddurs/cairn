---
id: f961f3f0-ee94-4ff6-aaa3-4ccc843ffa4c
title: Make the approved queue explicit to agents
type: feature
status: done
milestone: v0.3
assignee: codex
owner: oddurs
created_by: codex
created: 2026-09-23
updated: 2026-09-23
closed_at: 2026-09-23
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

- [x] A fixture with renamed statuses, an untriaged idea, external waiting work, a dependency, and an approved item demonstrates the intended selection.
- [x] CLI next, atomic claim-next, MCP, and generated instructions follow the same documented policy.
- [x] Projects without an explicit policy retain the documented existing behaviour.
- [x] The policy composes with a caller's narrower filter and does not silently broaden an agent's scope.
- [x] The work includes a decision on whether configuration is necessary, backed by an observed session.

## Boundaries

No scheduler, background worker, hardcoded planned status, or new command just for this project.

## Released by codex

Ship the already-landed maintenance fixes first, as required by the v0.3 release sequence; then implement the approved-view contract.

## 2026-09-23

Observed this project selecting work with a saved next view, then duplicating its status filter in claim and handwritten agent instructions. Reuse the existing view explicitly in next, atomic claim-next, MCP, and agent generation. No new config key or format version: bare commands retain their established behavior. The view controls membership; next retains its documented active-first priority ranking.

## 2026-09-23

Implemented explicit saved-view selection across CLI, atomic claim-next, MCP and generated instructions. Six behavioral tests cover renamed statuses, external waits, dependencies, narrowing, unknown views, concurrent claims, explicit-assignment refusal, and regeneration. Observed the live next view returning 0136 first, then 0137 and 0138, with ideas and external waits excluded. No new configuration setting: existing views suffice. Full make check is running as the handoff gate.

## 2026-09-23

Full make check passed after the targeted suite: all tests, fmt, clippy with warnings denied, and 144 items validated with zero warnings. The live project now uses the view for selection and generation, including its existing active work.

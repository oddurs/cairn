---
id: 137
title: Keep Cairn and Harrow on the same contract
type: bug
status: planned
milestone: v0.3
owner: oddurs
created_by: codex
created: 2026-09-23
updated: 2026-09-23
priority: p1
effort: m
area: integration
---

## What happens

On 2026-09-23, Cairn at 3d67f0c and the installed Harrow read this repository successfully and agree on its item count. Harrow's two optional agreement tests also pass. That is useful, but covers only the filters in that test.

A disposable minimal project containing a closed item reproduced a gap: Cairn accepts `list --all --filter 'closed_at>=2026-09-01'`, while `harrow --plain --filter 'closed_at>=2026-09-01'` exits 2 with "no such field". Harrow at c61e37e stores this as an unknown field rather than a known built-in, and its vendored corpus is from Cairn v0.2.0, taken 2026-09-12.

## Proposal

Keep separate implementations and one public contract. Exercise the current corpus and selected query/semantic fixtures in both projects, with pinned versions and a deliberate update process. Expand from parsing to categories, grouping, dependencies, criteria, proposals, and known built-ins. Only change Harrow in its own repository and own item; this Cairn item coordinates the shared acceptance gate.

## Acceptance criteria

- [ ] Harrow can read, display, and filter closed_at, including a closed item edited on a later date.
- [ ] Agreement checks cover the views used by Cairn's own project and compare item sets through machine output.
- [ ] A CI job or explicit release gate runs against a pinned counterpart and cannot silently skip the comparison.
- [ ] Corpus provenance and a supported-version policy make mismatches diagnosable.
- [ ] The documentation identifies intentional differences such as free-text search and unknown-field handling.

## Boundaries

Do not add a shared Rust library, second writer, or mandatory Cairn process for every Harrow read.

---
id: 5c669bec-412a-4c3e-be78-679ec5ea9f33
title: Keep Cairn and Harrow on the same contract
type: bug
status: done
milestone: v0.3
owner: oddurs
created_by: codex
created: 2026-09-23
updated: 2026-09-23
closed_at: 2026-09-23
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

- [x] Harrow can read, display, and filter closed_at, including a closed item edited on a later date.
- [x] Agreement checks cover the views used by Cairn's own project and compare item sets through machine output.
- [x] A CI job or explicit release gate runs against a pinned counterpart and cannot silently skip the comparison.
- [x] Corpus provenance and a supported-version policy make mismatches diagnosable.
- [x] The documentation identifies intentional differences such as free-text search and unknown-field handling.

## Boundaries

Do not add a shared Rust library, second writer, or mandatory Cairn process for every Harrow read.

## 2026-09-23

Harrow work is in its own fix/cairn-contract worktree and item 0099. Expanded comparison found and repaired additional query mismatches; its full local check, refreshed 36-case corpus, recorded-date UI/statistics regression, and three explicit cross-tool tests pass. All eight live Cairn views agree through machine output. Required pinned-counterpart CI is now being submitted; next add the reciprocal Cairn gate.

## Released by codex

Companion implementation is verified locally and awaiting required PR CI plus the reciprocal pinned gate. Continuing with the independent Git workflow slice.

## 2026-09-23

Harrow PR #93 merged as e7397a506b244e250921b2d87f1a89102d43c9d9 after required Linux, macOS and pinned-Cairn agreement CI passed. Cairn now pins that exact commit in spec/harrow-revision; make agreement passes all three cross-tool checks and refuses a missing fixture variable. Reciprocal CI uses that same gate. doc/COMPATIBILITY.md and Harrow COMPATIBILITY.md document provenance, supported revisions and intentional differences. No shared runtime library or second writer was introduced.

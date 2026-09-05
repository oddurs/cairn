---
id: 10
title: Define an interchange format for import and export
type: feature
status: done
milestone: v0.2
created: 2026-09-04
updated: 2026-09-04
priority: p1
effort: m
---

## Problem

`0003` and `0004` are written as GitHub features. Building them that way puts
one proprietary forge in the core and leaves every other source — GitLab,
Savannah, git-bug, a CSV someone exported from Jira — as a special case.

## Proposal

Define one documented JSON interchange format and make every integration an
adapter over it.

    cairn import --from github --repo owner/name
    cairn import --from json < items.json
    cairn export --to json

`cairn import --from json` is the core; `--from github` becomes a thin adapter
that produces that JSON. Anyone can write an adapter for any tracker without
touching cairn, and a tracker cairn has never heard of is one `jq` script away.

The format is essentially `cairn list --json` with a schema descriptor attached,
so most of it exists already.

## Acceptance criteria

- [ ] The interchange format is specified in the manual
- [ ] `cairn export --to json` and `cairn import --from json` round-trip losslessly
- [ ] Importing maps foreign statuses onto local ones, reporting what it could not map
- [ ] `0003` and `0004` are implemented as adapters, not as core code

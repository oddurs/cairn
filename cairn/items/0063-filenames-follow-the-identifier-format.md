---
id: 63
title: Filenames follow the identifier format
type: feature
status: backlog
milestone: v0.2
created: 2026-09-06
updated: 2026-09-06
priority: p0
sprint: s6
effort: s
---

## Problem

A project whose identifiers read `MP-1002` should have files called
`MP-1002-support-oauth-login.md`. Otherwise the name and the identifier disagree,
which is exactly the confusion the key was adopted to remove.

Two consequences follow, and both need handling rather than discovering.

## Adopting a format renames every file

`cairn check` already warns when a filename does not match its title, and the
same machinery notices a filename that does not match the identifier format. So
the warning arrives on its own; what is missing is the thing that fixes it in
one step rather than by touching every item.

## Reading an identifier back out of a filename

The specification allows a reader to take the identifier from a leading run of
digits when the `id` key is absent. `MP-1002-slug.md` has no leading digits, so
that fallback stops working for a project with a key.

cairn always writes `id`, so its own files are unaffected. But a hand-written
file in such a project would be unreadable, and silently — which is the wrong
failure. The fix is to parse the filename through the same template, and to say
in the specification that the digits fallback applies to the default naming
only.

## Acceptance criteria

- [ ] New items are named using the configured format
- [ ] One command brings an existing project's filenames into line
- [ ] `check` reports a filename that does not match, as it already does for titles
- [ ] A file with no `id` key is read through the configured format
- [ ] A project without a key behaves exactly as it does today

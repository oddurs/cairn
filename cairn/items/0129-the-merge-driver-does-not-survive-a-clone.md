---
id: 129
title: The merge driver does not survive a clone
type: bug
status: backlog
milestone: v0.1
created: 2026-09-19
updated: 2026-09-19
priority: p1
area: git
effort: s
---

## What happens

A merge driver is defined in `.git/config`, which is not cloned. `.gitattributes`
is. So a project set up with `cairn init --git` commits a file saying *use the
cairn merge driver* and every person who clones it has no such driver defined.

The failure is silent and one-sided: it works for whoever ran `init --git`, and
does not work for anybody else, so the person who could fix it never sees it.

Measured. Two branches, each adding one item, both allocating `0002`:

```
# with the driver installed
cairn: the merge changed identifiers or the roadmap; review and commit:
   M ROADMAP.md
   D cairn/items/0002-bob-s-work.md
  ?? cairn/items/0003-bob-s-work.md
```

That is the behaviour this project built, and it is good. In a fresh clone of the
same repository there is no driver, the merge is left to git, and the next thing
the newcomer sees is:

```
cairn: cairn/items/0002-bob-s-work.md:2: id 0002 is used by 2 files
  (cairn/items/0002-alice-s-work.md, cairn/items/0002-bob-s-work.md)
  — run `cairn renumber`
```

`renumber` is the right advice for the symptom and says nothing about the cause.
Only `cairn config` mentions the real problem, and only if you already suspect
it:

```
git          not integrated — `cairn init --git`
```

## What should happen

The mismatch is detectable with no guessing: `.gitattributes` asks for a driver
by name, and `git config merge.cairn.driver` is unset. When a repository is in
that state, say so, once, with the command that fixes it — and say it where
somebody will be standing, not only in `cairn config`.

`cairn check` is the obvious place: it already runs in CI and before finishing
work, and a duplicate id is already an error it reports. Reporting the cause
beside the symptom is the whole fix.

`git::is_configured` already checks `.gitattributes` and the `post-merge` hook.
It does not check the driver definition, which is the half that does not survive
a clone.

## Reproduction

1. `cairn init --bare --git` in a git repository, commit
2. Two branches, each `cairn new`, each commit
3. `git clone` the repository somewhere else
4. `git config --local --get-regexp merge.cairn` — nothing
5. Merge the branches in the clone: no renumbering, duplicate ids

## Why it matters

0066 wants three projects that are not this one using cairn for a fortnight. In
any of them with more than one contributor, the second person hits this, and what
they learn about cairn is that a shared backlog produces merge conflicts. That is
the opposite of the thing being demonstrated.

## Acceptance criteria

- [ ] A repository whose `.gitattributes` names a driver that is not defined is told so
- [ ] The message names the command that fixes it
- [ ] `cairn init --git` is safe to run again on a project that is already set up
- [ ] Said once, and not by every command in a loop
- [ ] Nothing is said when the driver is defined, or when the project is not a repository
- [ ] A test clones a configured project and asserts the newcomer is told

---
id: 56
title: Collect a contributor licence agreement
type: chore
status: backlog
milestone: v0.2
created: 2026-09-05
updated: 2026-09-05
priority: p0
effort: s
---

## Problem

`git log` shows one contributor. That makes the maintainer the sole copyright
holder, and therefore free to offer cairn under other terms — dual-licensed, or
a commercial licence alongside the GPL — should that ever be wanted.

The first outside patch merged under the GPL ends that, permanently. Undoing it
afterwards means finding every contributor and asking permission, and one
refusal or one unreachable person freezes the project's options for good.

`CONTRIBUTING.md` currently says there is no copyright assignment, which was
written without anybody having considered the question.

## Proposal

A contributor licence agreement, not an assignment. The contributor keeps their
copyright and grants a licence broad enough that the project can be
relicensed. Apache's individual CLA is the well-trodden shape and includes the
clause covering contributors whose employer may hold rights.

Checked automatically on each pull request, with signatures recorded in the
repository rather than a third-party service — consistent with a project whose
whole argument is that state belongs in the repository.

## Why this and not assignment

Assignment transfers ownership and asks a contributor to give something up. It
deters people, and the earlier advice in this project to avoid it was right for
a pure free-software project. A licence agreement costs the contributor nothing
they will miss and keeps every option open.

## What contributors are owed

The reason for the agreement, stated plainly in `CONTRIBUTING.md`: the
maintainer may offer cairn under terms other than the GPL. Somebody signing
should know what they are signing and why, rather than finding out later.

## Acceptance criteria

- [ ] The agreement grants copyright and patent licences, and covers employers
- [ ] The GPL remains the licence cairn is distributed under
- [ ] A pull request cannot merge without agreement, checked automatically
- [ ] Signatures live in this repository
- [ ] CONTRIBUTING says why, without euphemism
- [ ] Reviewed by a lawyer before it is relied on

## 2026-09-05

Drafted and wired up.

`CLA.md` follows the Apache individual CLA: copyright licence, patent licence
with a retaliation clause, the representations about original work and about an
employer's rights. The contributor keeps their copyright — it is a licence, not
an assignment.

The check runs on every pull request and records agreement in
`.github/contributors.json` in this repository, using the workflow's own token
rather than a third-party service, which is the same argument cairn makes about
everything else.

`CONTRIBUTING.md` now says plainly why it exists: the maintainer is currently
the sole copyright holder and can therefore offer cairn under other terms; the
first contribution merged without an agreement ends that permanently. Somebody
being asked to sign is entitled to know what for. It also says what to do if
they would rather not.

Still open: a lawyer has not read it. The adaptation is from a standard
template, but "adapted from something standard" is not the same as reviewed, and
the file says so.

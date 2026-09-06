---
id: 59
title: 'Declare cairn''s boundary: what one repository can do'
type: docs
status: backlog
milestone: v0.2
created: 2026-09-06
updated: 2026-09-06
priority: p0
effort: s
---

## Problem

"Free tier and paid tier" is a business decision, and every feature request
lands in the argument about which side it falls on. That argument has no
principle in it, so it gets settled by mood, and the answer changes depending
on who is asking and when.

There is a line here that is not a business decision at all:

> cairn does everything a single repository can do. Anything else is somebody
> else's problem.

That is a fact about what a directory of files is, not a policy. A repository
cannot see other repositories. It cannot serve somebody who has not cloned it.
It has no notion of who is permitted to close an item. It cannot tell you
something changed while you were not looking.

Those are the things a hosted product would exist for, and none of them is a
capability withheld from cairn. They are capabilities a repository does not
have.

## Why it matters for the design, not just the marketing

A stated boundary answers feature requests by principle. "Can cairn notify me?"
— no, and not because of a tier: a program that runs when you invoke it cannot
notice anything while you are not running it. "Can cairn show me work across my
repos?" — no; run it in each, or use something that reads the format.

It also protects the tool from the failure mode where the command line slowly
becomes a client for a service. The guard is concrete: **cairn never gains a
concept of accounts, authentication, or remotes.** The moment `cairn login`
exists, the argument is lost.

## Acceptance criteria

- [ ] The boundary is stated in the manual as a design constraint
- [ ] The three things cairn will never grow — accounts, authentication, remotes
      — are named explicitly
- [ ] Feature requests can be answered by pointing at it
- [ ] It reads as a description of what a repository is, not as a pricing page

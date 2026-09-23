---
id: 66
title: Find three people who are not the author
type: chore
status: blocked
milestone: v1.0
owner: oddurs
created: 2026-09-06
updated: 2026-09-23
priority: p1
area: workflow
effort: m
---

## Problem

Nine and a half thousand lines of Rust, twenty-eight commands, a specification,
a website, sixty-five items — and nobody outside this repository has ever run
it.

That is not a gap in the software. It is the reason every other gap is hard to
prioritise: without users, "what should we build" has no evidence behind it and
gets answered by whoever is most articulate, which so far has been whoever was
writing at the time.

The evidence for this is already in the repository. Eleven real defects have
been found so far, and roughly none of them came from tests written in advance.
They came from a soak test, from continuous integration on a platform nobody
here can run, from an adversarial review, from actually attempting a release,
and — twice, within a single day — from putting cairn into one other project.
The two that came from real use were both about *what the tool assumes people
mean*, which is the category no amount of testing finds.

## Proposal

Three projects, not the author's, using cairn for their actual backlog for at
least a fortnight. Not a demo, not a star: somebody who closes items on a
Tuesday because they finished something.

What to do with them is the whole point — watch, do not ask. What did they
configure first? What did they get wrong? What did they stop using? Which
command did they type that does not exist? File what is learned, whatever it
contradicts.

## The rule this exists to enforce

**After any two features, the next thing is a user rather than a third
feature.** Written into `CONTRIBUTING.md`, so it constrains whoever is working
rather than depending on their mood.

## Acceptance criteria

- [ ] Three projects outside this repository, using it for real work
- [ ] Two weeks each, minimum
- [ ] What each did in their first hour is written down
- [ ] Everything learned is filed, including anything that contradicts the
      current roadmap
- [x] The two-features rule is in CONTRIBUTING

## 2026-09-23

Assessment 0134: the author's weeks of daily use are real evidence and supersede the original claim that nobody uses the tool. Independent feedback remains valuable. Recruitment is a human task; waiting here means finding three willing outside projects, not waiting for more features. The two-feature pause is now stated in CONTRIBUTING. Record observations and follow-up items; do not contact anyone automatically.

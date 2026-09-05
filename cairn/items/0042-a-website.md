---
id: 42
title: A website
type: docs
status: done
milestone: v0.2
created: 2026-09-05
updated: 2026-09-05
priority: p1
effort: m
---

## Problem

There is nowhere to send someone. The README is good and it is inside a git
repository, which means the audience is already people who found the repository.
Adoption is this project's bottleneck, and a tool with no front door is one
people hear about and then do not try.

## What it is not

Not the web board that `0007` was dropped for. That was a second place to look
at your backlog, which contradicts the whole design. A static page describing a
program contradicts nothing — it is how anyone finds a command-line tool.

## Proposal

Astro, statically built, on GitHub Pages. No server, no account beyond the one
already hosting the repository.

One page that does the job:

- The artefact first. An item is a Markdown file; showing the file is a better
  argument than describing it.
- The recorded demo, which is generated from real command output and therefore
  cannot drift into flattery.
- The agent story, which is the differentiator.
- The install command, working.

Plus the format specification, rendered from `spec/README.md` so there is one
source and no second copy to fall out of date.

## Acceptance criteria

- [ ] Every code sample is real output from the program, not illustrative
- [ ] The specification page renders from spec/README.md; no duplicated prose
- [ ] Builds and deploys from CI on push
- [ ] Legible on a phone, keyboard-navigable, respects reduced motion
- [ ] Loads without JavaScript

## 2026-09-05

Built: Astro, static, GitHub Pages. A landing page, four documentation pages, and the format specification rendered from spec/README.md so there is one authority. The recorded demo is synced from doc/demo.svg for the same reason — a site that restated either would eventually contradict the first and flatter the second.

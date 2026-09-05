---
id: 44
title: Back the site with a design system
type: chore
status: done
milestone: v0.2
created: 2026-09-05
updated: 2026-09-05
priority: p1
effort: m
---

## Problem

The site is Astro, but nothing sits underneath it. `global.css` holds a handful
of custom properties and then every page re-declares `.cta`, `.actions`,
`.band` and its own spacing in an inline `<style>` block. Six inline blocks,
the same button defined twice, magic `clamp()` values scattered through the
pages.

That is a stylesheet, not a system. It will drift the moment a second person
touches it, and it makes the docs harder to write than they should be: each
page is JSX with prose threaded through it, when the content is plainly
Markdown.

## Proposal

Two layers of tokens — raw values, then semantic names that pages actually use
— and a set of primitives that compose, so a page declares content rather than
CSS.

- Colour, space, type, border and radius scales, each with one definition.
- `Band`, `Prose`, `Button`, `Stack`, `Grid`, `Callout` alongside the existing
  `Terminal`.
- Documentation pages become Markdown in a content collection, with components
  available where a page needs one. Writing a doc should be writing prose.
- A page documenting the system, which for a documentation site earns its place
  and doubles as somewhere to notice when something looks wrong.

## Acceptance criteria

- [ ] No page declares a colour, a spacing value or a font size directly
- [ ] Every recurring pattern exists once
- [ ] Documentation is Markdown, not JSX
- [ ] The system is visible on its own page
- [ ] Still loads without JavaScript, still keyboard-navigable

## 2026-09-05

Two token layers — raw stone values, then semantic names for their jobs — and
eight components. Pages compose; they no longer declare colour, spacing or type.
The audit is clean: no hex outside the token file, no hardcoded spacing or font
sizes in pages, and every recurring pattern defined once.

Documentation moved to a content collection of MDX, so writing a document is
writing prose with a component available where one is genuinely needed. Four
pages of JSX became four Markdown files and one dynamic route.

The system documents itself at /design, drawn with the tokens it names — so
when something is wrong there, it is wrong everywhere.

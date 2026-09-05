---
id: 8
title: Embed GNU Guile as an extension language
type: feature
status: backlog
milestone: later
created: 2026-09-04
updated: 2026-09-04
priority: p1
effort: xl
---

## Problem

The GPL grants the right to modify cairn; the architecture does not grant the
ability. Changing how the board groups, how the roadmap renders, or what counts
as a valid item all require forking a Rust program and recompiling.

The symptom is visible in `[render]`, which has accumulated eight booleans —
`checkbox`, `show_ids`, `progress`, `group_by_status`, `link_items` and the
rest. That accretion is what a program looks like when every user need has to
be anticipated by its author because users cannot express their own.

Hooks (shipped) cover the *reactive* half: run my program when something
happens. They do not cover the *substitutive* half: replace how cairn itself
does something.

## Proposal

Embed GNU Guile, loaded from `.cairn/init.scm` in the repo and
`~/.config/cairn/init.scm` for personal defaults.

    (define-validator (needs-owner item)
      (or (not (active? item)) (assignee item)
          (error "active items need an assignee")))

    (define-renderer (roadmap items)
      ...)                          ; replaces the built-in, not configures it

    (define-command (blocked)       ; a new subcommand, in userland
      (for-each show (filter (lambda (i) (any open? (deps i))) (all-items))))

Guile specifically: it is the GNU project's official extension language, it is
designed to be embedded, and its licence (LGPL) is compatible.

## Cost, honestly

- libguile is a large C dependency. `cargo install cairn` stops producing a
  self-contained static binary and starts requiring system Guile.
- Two ways to do everything, for a period, while built-ins are ported to
  Scheme.
- A Scheme API is a compatibility surface. Once published it constrains
  internals the way the file format constrains storage.

## Open questions

- Optional `guile` cargo feature, so the plain build stays dependency-free?
  Likely yes — but then extensions are not portable between installs, which
  undercuts the point.
- Does the built-in renderer get *ported* to Scheme, or merely made
  overridable? Porting is the honest version and the larger job.

## Acceptance criteria

- [ ] Validators, renderers and commands can all be defined in Scheme
- [ ] At least one built-in is implemented in Scheme rather than Rust
- [ ] `[render]`'s boolean knobs can be deleted without losing capability
- [ ] Building without Guile still produces a working program

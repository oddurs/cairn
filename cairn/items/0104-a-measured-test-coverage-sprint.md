---
id: 104
title: A measured test coverage sprint
type: chore
status: done
milestone: v0.1
created: 2026-09-07
updated: 2026-09-07
priority: p1
sprint: s10
effort: xl
area: testing
---

## Problem

The suite was measured rather than guessed at: `cargo-llvm-cov` put it at
**85.68% of regions and 87.91% of lines**, and the uncovered ranges named whole
commands nobody had ever run in a test — `board --group-by`, `roadmap --items`,
`set --filter`, `log --patch`, `get_schema`, the rendered roadmap's header and
footer, the import update path, every colour name the configuration accepts.

Coverage as a number is not the point. The point is that an uncovered line is a
line whose behaviour nobody has ever checked, and this project's own history
says that is where the defects are: every one of the last dozen was found by
writing a test for something that had none.

## Proposal

Wave after wave, each one measured, each targeting the largest blocks of
uncovered regions rather than whatever is easiest to reach. Stop when the
remaining gaps are error paths that cost more to reach than they are worth.

Not a target percentage. A target *list*: the surfaces a person actually uses
and nothing had driven.

## Acceptance criteria

- [x] Coverage measured before and after, with the tool recorded
- [x] Every command's flags exercised at least once
- [x] Each defect the sprint turns up is fixed with a test that fails without it
- [x] Dead code found along the way is removed rather than covered
- [x] `make durability` passes at the end

## 2026-09-07

Done. 85.68% to 91.27% of regions, 87.91% to 93.10% of lines, measured with cargo-llvm-cov before and after; 227 to 301 end-to-end tests. Five defects, none of which a coverage number would have found on its own: `board --group-by milestone` silently dropped every scheduled item, because the context was built after containers were filtered out; `owner` and `created_by` were writable and shown as columns but absent from every machine-readable shape, so an export dropped them and a round trip lost them — and the specification never documented either key, so a second implementation would have dropped them too; `refs::validate_on_write` covered only *declared* refs, so `depends_on` was exempt from the rule its own comment claimed it enforced; `cairn new` checked for a cycle but not for a reference to nothing; and `renumber::apply` carried eighteen lines of unreachable reference-rewriting behind an argument that was empty at its only call site.

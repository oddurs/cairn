---
id: 162
uid: ccb1b84d-72a4-4d71-926e-e11b8896e3df
title: Advance the Harrow pin to Results and walk the agent loop across both tools
type: chore
status: done
milestone: v1.0
assignee: oddurs
created: 2026-09-28
updated: 2026-09-28
closed_at: 2026-09-28
priority: p1
effort: s
area: integration
---

## Why

spec/harrow-revision pins Harrow f59a46a, from before Harrow read a `## Result`
(its 0118, specification §10.2). Harrow's main, 0ae9c09, reads it, and cairn
main now writes it (0155) and compiles prompts that carry it (0154). Until the
pin moves, `make agreement` checks Cairn against a Harrow that has never seen
a Result.

## Approach

1. Pin 0ae9c09 in spec/harrow-revision. harrow-51 agreed this; Harrow pins
   cairn 2f41e9a in return.
2. Run `make agreement` against a clean checkout of 0ae9c09.
3. Walk the agent loop in a scratch project with cairn main and Harrow at the
   pin: next, prompt, claim, note, tick, close --result. After each step,
   check that Harrow reads the same item, and at the end that it reads the
   result cairn wrote.

## Acceptance criteria

- [x] spec/harrow-revision pins 0ae9c09 and `make agreement` passes against it
- [x] The loop runs end to end, and Harrow's reading of the closed item carries the same result as `cairn show --json`

## 2026-09-28

make agreement HARROW_REPO=<detached worktree of harrow at 0ae9c09> passed 5/5 (harrow_and_cairn_select_the_same_items, the_rule_about_what_an_ordinary_listing_hides_agrees, the_cairn_projects_saved_views_agree_through_machine_output, a_field_neither_tool_declares_is_refused_here, migrated_numbers_renderings_and_tags_agree) with cairn main 2f41e9a.

## 2026-09-28

Walked the agent loop in a scratch git project with cairn 2f41e9a, reading Harrow's side through its own library at 0ae9c09 (a probe calling harrow::engine::Project::load and harrow::item::result_of). Filed A and B depending on A, then next, prompt A, claim, note, tick 1, tick 2, close --result. After every step Harrow and cairn show --json agreed on status, assignee, criteria done/total and result for all 6 items. next offered A and not B until A closed; B's prompt then carried A's result; Harrow read A's result as the exact text close wrote. The same comparison over cairn's own backlog: 162 items, 7 with a Result, 0 disagreements. Harrow cannot show a Result yet (its 0119), and its vendored corpus has no Result cases until its 0120; neither is on this side.

## Result

Harrow is pinned at 0ae9c09, which reads a Result as specification 10.2 does. make agreement passes against it, and the agent loop (next, prompt, claim, note, tick, close --result) leaves Harrow and cairn agreeing at every step, down to the result text a dependent's prompt quotes.

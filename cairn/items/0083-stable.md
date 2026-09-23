---
id: 83
key: v1.0
title: Stable
type: milestone
status: backlog
depends_on:
- 135
created: 2026-09-07
updated: 2026-09-23
---

Safe to keep: an explicit compatibility contract, tested recovery, useful retrieval of old decisions, and verified releases. The gate is evidence from daily use and long-lived data, not a calendar date or a larger feature count.

## Release gate

- Complete the daily-loop outcome in v0.3.
- Record outside-use evidence through 0066 and adjust the contract where it contradicts our assumptions.
- Settle identifier policy in 0067 using branch/worktree evidence and independent review.
- Demonstrate retrieval from old work in 0139 and durability over long histories in 0140.
- Resolve the release-verification policy in 0052; keep the binary and Homebrew paths independently verified.
- Review CLI, JSON, MCP, configuration, and item-format promises together with Harrow compatibility.

Additional package ecosystems are useful when there is demand, but do not define 1.0. The old 2027-03-01 date was removed during assessment 0134 because it was not supported by a real release commitment.

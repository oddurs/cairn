---
id: 134
title: Give Cairn a durable direction and a working project setup
type: chore
status: done
milestone: v0.3
assignee: codex
created_by: codex
created: 2026-09-23
updated: 2026-09-23
closed_at: 2026-09-23
priority: p1
effort: l
area: direction
---

## Purpose

Assess Cairn and its Harrow counterpart after several weeks of daily use. Establish a lightweight, Git-native, agent-first direction, reconcile the existing backlog with the shipped product, and configure this repository so the next work is clear.

## Acceptance criteria

- [x] Assess the implementation, contracts, tests, documentation, and Harrow integration with concrete evidence.
- [x] Record a durable product concept, boundaries, and a sequenced recommendation in this item.
- [x] Reconcile current open work and milestone definitions without erasing historical reasoning.
- [x] Configure useful views, templates, and roadmap generation, and refresh agent instructions.
- [x] Verify the resulting setup and run cairn check successfully.

## Assessment — 2026-09-23

Cairn should become **project memory, versioned with the code**. Its enduring
value is a record that a person or a new agent can recover after the original
conversation, model, editor, and author have gone. Work selection and a roadmap
make that memory actionable. This is a sharpening of 0071, not a new storage
system or a reason to make every small task an essay.

The author's weeks of daily use are the strongest product evidence available
in this session. The implementation is already substantial and useful. The
largest opportunity is coherence: make the experience, repository setup,
integration contracts, and promises say the same thing.

### What I inspected

Cairn at 3d67f0c and the neighbouring Harrow checkout at c61e37e: configuration,
item parsing and writes, lock/recovery paths, queries and readiness, claims,
MCP dispatch, Git integration, rendering, tests and fixtures, release/CI
workflows, README/manual/site, and the complete open backlog. Harrow's
schema/reader, filter agreement tests, conformance provenance, documentation,
and actual rendering against this project were inspected. This is a product
and architecture assessment with targeted reproductions, not a claim that
every path on every supported platform has been audited.

### What is already worth protecting

- The item files are useful on their own. The specified format, frozen older
  corpora, independent Python reader, and direct Harrow reader give the data
  life beyond this binary. That is a stronger foundation than a particular UI.
- One write authority keeps validation, locking, hooks, and IDs consistent.
  Harrow's choice to read directly and write through Cairn is sound. Keep it.
- Project-owned statuses, categories, fields, reference types, and hooks offer
  flexibility without requiring a plugin runtime or a particular methodology.
- Atomic writes, explicit format migrations, contention/fuzz/soak coverage,
  cross-platform CI, and a declared machine-output contract are real assets.
  The baseline test run passed 538 tests, with two soak tests intentionally
  ignored in the ordinary suite.
- Claims, handoff reasons, proposals, criteria, and bounded JSON are useful
  agent primitives already present. Adding an agent runner would duplicate
  somebody else's responsibility and make the core harder to maintain.
- Harrow already has the right human questions: what changed, what needs me,
  and how the backlog stands. Invest in their correctness and clarity before
  adding more screens.

### Where the experience and the claims diverged

| Evidence | Implication | Action |
| --- | --- | --- |
| Before this assessment: 133 items, 116 done, six dropped, seven unfinished work items, four milestone items; both saved next and sprint views were empty | The project had historical activity but no selected engineering queue | Select three concrete next items and make the roadmap show unfinished work |
| v0.2.1 is tagged, but v0.1 and v0.2 milestones were still open with future dates | The roadmap described an obsolete plan rather than the product | Reconcile the historical outcomes and remove invented deadlines |
| 0001 was doing with no claimant although its body described a credential wait | The highest-ranked next action was not executable engineering work | Mark external waits honestly, assign an accountable owner, and scope autonomous claims |
| Ctx::is_ready checks completion and dependencies, not project approval or a status named blocked | Dependency-ready is a narrower promise than approved to start | Document the distinction now; 0136 evaluates one shared policy |
| Both claims succeeded for one item in two linked worktrees in a disposable repo | The lock and claim guarantee is local, not distributed | Narrow the documentation and prove the full supported workflow in 0138 |
| Harrow doctor agreed on the item count and two optional agreement tests passed, but closed_at filtering failed with exit 2 | Agreement on counts and a small filter set is insufficient | Reproduction and an integration gate in 0137 |
| Harrow's conformance provenance names Cairn v0.2.0 from 2026-09-12 | Independent implementations need an explicit update/release contract | Keep separate readers; test the shared format and semantics together |
| README duplicated its import section, included obsolete milestone configuration, and omitted Harrow from its story | Good software was harder to understand than it needed to be | Rewrite the entry point; retain the manual as the detailed reference |
| CONTRIBUTING still said migrate was hidden and counted an old command surface | Historical arguments had become misleading present-tense instructions | Correct the current workflow and rule descriptions |
| make check invoked target/release/cairn without building it; the file was absent in this checkout | The advertised contributor command depended on unmentioned prior work | Make build an explicit prerequisite |
| This clone had tracked merge attributes but no registered driver | Even this project's own Git setup was incomplete | Run cairn init --git locally and document clone setup |
| First-use checks found an invalid bare-init hint and a keyless milestone used as ordinary example work by the minimal preset | Newcomer paths need to be exercised as workflows, not just parsed as valid arguments | Fix the hint in 0143; retain the independently reproduced example-item bug as triage item 0144 |

The separate-worktree result matches Git's model: linked worktrees can check
out different branches and have their own working files. See the official
[git-worktree documentation](https://git-scm.com/docs/git-worktree). A UUID
would reduce identifier collisions; it would not turn branch-local claims into
global reservations. Those are distinct design questions.

Two additional durability questions came from inspection, not reproduced
failures: age-only breaking of a lock after 300 seconds, and staged-renumber
recovery during a read. 0140 records them as things to establish under
contention before asserting a stronger guarantee.

### The concept and its boundaries

There is one durable record: a directory of items and its schema. The item
holds intent, evidence, and outcome. Git holds how that record changed. Cairn
makes it queryable and safe to change. Harrow helps a person read it and steer
the work. CLI/JSON and MCP let an agent participate in the same workflow.

This gives a practical test for every addition: does it help someone capture,
find, act on, review, or recover that record? If a schema declaration, existing
query, shell hook, or separate consumer does the job, start there. The cost of
a new feature includes its compatibility promise and documentation ten years
from now.

Keep the format ordinary, core operations local, automation optional, and Git
responsible for version control. Keep the no-library decision: a shared Rust
API would create another compatibility surface without replacing the need for
an independently readable format. Keep a small working vocabulary visible at
the entrance even though the complete command reference is broad.

Do not spend this cycle on hosting, accounts, remote reservations, embedded
scripting, an agent orchestrator, a web application, embedding-based memory,
or machine-wide project aggregation. Some could become useful independent
consumers. None is necessary to make the existing promise hold.

Also avoid process weight: no new hierarchy levels, estimates, sprint schedule,
automatic commits, or mandatory proposal approvals. A plain task with a short
reason is sufficient when that is all the work needs. The new decision type
is optional schema, not a new built-in key or file format.

### What to do, in order

**Ship what already exists.** 0141 records the 0.2.2 maintenance release path.
The release owner should verify and publish the fixes already on main, without
waiting for the next product outcome or optional package ecosystems. Preparing
this assessment does not publish a release or configure secrets.

**v0.3: trust the daily loop.** The selected engineering slice is deliberately
three items: 0136 (approved work), 0137 (Cairn/Harrow agreement), and 0138
(branches and worktrees). Start with 0136. These can each produce a concrete
improvement with a regression check and a clearer contract. Observe outside
usage through 0066 alongside that work. Do not add a fourth selected feature
because an implementation slot opened before there is new evidence.

**1.0: be safe to keep.** After the daily loop, 0067 settles identity from
actual collaboration evidence, 0139 measures how a returning human or fresh
agent finds old reasoning, and 0140 tests years of data and interrupted work.
Resolve the verification policy in 0052. Review the written promises for
CLI/JSON/MCP, configuration, and format together. 1.0 means those contracts
are trustworthy; it does not mean a certain number of commands exists.

**Later is an option.** crates.io and wider distribution remain visible in
0001/0053, with the work still required stated honestly. They are useful reach,
but should not outrank everyday correctness. 0127's proposed relaxation of
durability is declined on its own later measurements, with the unmet tests
left unticked. Integer IDs remain unchanged while their decision stays open.

There are no new release dates. The old dates had no corresponding deadline.
Select the next outcome, learn from it, and then choose the next slice.

### What a good year would look like

Measure use and recoverability rather than feature count. An agent starting
with no conversation history should select approved work, find relevant old
decisions, and leave a reviewable result. A person reopening the repository
after a month should understand what needs attention without reading the
whole backlog. A second reader should agree with Cairn. Two contributors
should know what happens to their items across a branch and merge. Old files
should stay readable even when the original tools are gone.

For the next release, record success and failure on these tasks in actual
projects. For 1.0, keep a reproducible backlog with 1000 and 10000 mostly
finished items and publish named-hardware timings for read, search, render,
and Harrow startup. Establish baselines before setting performance thresholds.
No target justifies weakening acknowledged-write durability without a separate
decision backed by evidence.

### Setup implemented in this assessment

The repository now has meaningful now/next/waiting/dependencies/triage/later/
decisions/history views; three selected engineering items; templates for work,
bugs, docs, and decisions; a seven-day stale-claim notice; portable argv hooks;
and an open-work roadmap with a short introduction. The historical sprint
field remains readable but is no longer assigned or shown in every row.

The README is a shorter entrance with a working small-schema example, an
explicit role for Harrow, and truthful local-claim semantics. The manual and
site introduce the same concept. CONTRIBUTING and the project-specific portion
of AGENTS describe the scoped queue, one active item per worker, the distinction
between waiting and dependencies, and evidence before more features.

Old item bodies remain intact. Declined work is dropped rather than deleted;
milestones close on the reconciliation date rather than fabricated release
dates. Global require_criteria remains off because historical closed items have
unticked checkboxes whose truth was not established here. Current work still
requires verified criteria before closure. History is available through saved
views, search --all, and Git; it no longer overwhelms the forward roadmap.

These changes configure Cairn using the existing model. The build change makes
make check build the binary it invokes. First-use verification also found and
corrected an init --bare hint that named a nonexistent milestone (0143).
No new runtime dependency, command, format version, or service is introduced.
Harrow's repository remains unchanged; its integration gap is an explicit
next task, not reported fixed.

### Verification and limits

- `make check` passed after the init fix: 538 tests passed, two ordinary-suite
  soak tests ignored, formatting clean, Clippy with warnings denied clean,
  and strict roadmap validation passed.
- `make conformance` passed all 36 corpus cases across the current and frozen
  formats. `make doc` built the Info manual.
- `make audit` passed advisories, bans, licenses, and sources. It reported
  existing informational warnings for unused license allowances and the two
  transitive syn versions; no dependency changes were made.
- `npm ci` installed the locked site dependencies, reported zero known
  vulnerabilities, and `npm run build` generated all ten pages successfully.
- `harrow --doctor` accepted all eight views and agreed with Cairn on the
  repository's item count. The actual item sets matched for every configured
  view, and the next view was inspected as a terminal frame.
- Harrow's optional agreement suite passed both tests. The additional
  closed_at reproduction still fails, as recorded in 0137; passing the small
  existing suite is not evidence that every query agrees.
- The README's minimal/bare workflow and its complete TOML example were run
  in disposable directories. The editor handoff used a no-op editor for the
  automated smoke check. The corrected hints were followed in standard/bare,
  minimal/bare, and standard-with-examples projects; creation and strict
  validation passed. Minimal-with-examples independently exposed 0144.
- `make record` regenerated the demo, terminal samples, roadmap, and agent
  block. The recorded demo and samples did not change. `git diff --check`
  passed. Final item/roadmap validation is recorded below after closing.

This session did not run the long durability target, coverage job, remote CI,
or clean installs on every supported platform. It did not modify a durability
path, publish a release, push commits, contact prospective users, or change
Harrow. Those limits are deliberate: the reproduced future-work items remain
open rather than being implied complete by the new roadmap.

## 2026-09-23

Handoff: assessment, repository setup, documentation refresh, local Git integration, and the bounded init hint fix are complete. The next approved items are 0136, 0137, and 0138. External actions remain in the waiting view; the independently reproduced minimal-example defect is 0144 in triage. All changes are local and uncommitted.

## 2026-09-23

Final strict validation after reconciliation: 144 items, zero warnings. Future agent-filed work has an explicit project owner; assignee remains unset until someone claims it.

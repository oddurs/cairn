---
id: 128
title: A finished project looks like it contradicts itself
type: bug
status: done
milestone: v0.1
created: 2026-09-18
updated: 2026-09-18
priority: p1
area: list
---

## What happens

Reported from poptop: 70 work items done, 7 milestones still `status: backlog`,
`cairn roadmap` showing each at 100%. Three behaviours combine to make the
backlog look self-contradictory.

**1. `cairn list --status backlog` prints `no items match` while 7 items have
that status.** They matched, and were then hidden for being containers. The
message describes the filter, so the obvious response is to go and rewrite a
filter that was correct.

**2. Finishing a milestone's last item never says so.** The milestone stays in
`backlog` for ever unless somebody closes it by hand — and `backlog` is the
initial status, which the roadmap deliberately does not print, so the only
visible fact is 100%.

**3. The format-2 note does not say what migrating changes or whether it can be
undone.** Confirmed that closing a milestone in poptop really does require
migrating first.

## Proposal

**1 — keep the rule, fix the message.** Naming a status should not unhide
containers: `--status doing` would then list milestones, which is noise nobody
asked for, and the symmetry is deliberate — a status clause unhides closed items,
a type clause unhides containers. But `no items match` is false. Say what was
hidden and how to see it. The same lie exists for closed items, so name both.

**2 — report, never close.** Auto-closing would be cairn deciding that finished
work means a shipped milestone, and poptop's own `later / Someday` milestone has
all 3 of its items done and is meant to stay open. A `cairn check` warning would
nag about that for ever, which is the warning-fatigue failure this project has
already been bitten by twice. So: say it once at the moment it becomes true, on
`close`, and mark it on `roadmap`, which is where the state already is and so
where somebody who finished months ago will find it.

**3 — put the detail where somebody is stopped.** The recurring one-line notice
prints on every command and must stay short. The write refusal is reached once,
by somebody who now needs to know; so is `migrate --dry-run`. Say it there, and
only where it is true: a migration that rewrote existing items would not be
undone as cheaply as one that only rewrites `cairn.toml`.

## Acceptance criteria

- [x] An empty listing says how many items were hidden and how to see them
- [x] It still says `no items match` when nothing actually matched
- [x] Naming a type still shows containers; naming a status still does not
- [x] Closing the last item under a milestone says so, once, naming the command
- [x] Nothing is closed automatically, and `-q` says nothing at all
- [x] `roadmap` marks a milestone that is finished and still open
- [x] A milestone with nothing filed under it is never called finished
- [x] The write refusal and `migrate --dry-run` say what changes and how to undo it
- [x] A test builds the poptop state: all work done, milestones still in backlog

## 2026-09-18

Verified against a copy of poptop end to end: before, `list --status backlog` said nothing matched and the roadmap showed seven 100% bars with no sign any milestone was open. After migrating, the listing names the seven hidden milestones and the roadmap marks each finished-and-open one. Closing 0037 removes its mark and leaves the other six alone.

`later` is the case that settles point 2: it has 3 items, all done, and is meant to stay open for good. That rules out auto-closing and rules out a standing `check` warning; it does not rule out saying it once at the moment, or marking it where the state is already displayed.

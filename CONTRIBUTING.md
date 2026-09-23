# Contributing to cairn

cairn tracks its own work. Start with the selected queue and read an item's
reasoning before changing code. The [direction and assessment](cairn/items/0134-give-cairn-a-durable-direction-and-a-working-project-setup.md)
explain the current priorities; [ROADMAP.md](ROADMAP.md) shows unfinished work.

```sh
cargo build --release
./target/release/cairn list --view next  # the approved, dependency-ready queue
./target/release/cairn list --view waiting # external actions and decisions
./target/release/cairn show 23       # the reasoning behind an item
```

Items carry the thinking that produced them — the problem, the proposal, the
costs that were weighed, and acceptance criteria you can check yourself. If an
item's body does not tell you enough to start, that is a bug in the item; say so.

## How this project works

Cairn is project memory versioned with the code. Harrow is the human interface
for watching and triaging it; Git carries history and review. Keep the core
small: try a schema choice, query, hook, or external reader before expanding
the program. The next engineering slice is 0136–0138 in milestone `v0.3`.

| Status | Meaning here |
| --- | --- |
| `backlog` | Captured, but not committed to implementation |
| `planned` | Selected and described well enough to start |
| `doing` | Someone is actively working; claim it |
| `blocked` | Waiting for an external action or decision, recorded in the body |
| `done` | The stated outcome is verified |
| `dropped` | Deliberately declined; the reasoning remains useful |

Dependencies use `depends_on`. A status called `blocked` does not itself change
the computed dependency readiness. Bare `cairn next` ranks unfinished,
dependency-ready work, including ideas and external waits. For autonomous
selection in **this project**, use:

```sh
cairn claim --next --filter 'status=planned'
```

Keep at most three engineering items selected at a time and one active item
per worker. This is a working convention, not a hidden scheduler. Finish or
explicitly hand back the current work before taking more. A user-assigned task
can be claimed directly even when it was not in the selected queue.

`p0` means data loss or a broken core workflow, `p1` the next important outcome,
`p2` useful follow-up, and `p3` an option. Set `area` when it helps routing; do
not assign new `sprint` values, which describe the historical durability effort.
Release milestones have outcomes; add a `due` date only for a real commitment.

Use `cairn config` for the actual schema. The saved views are `now`, `next`,
`waiting`, `dependencies`, `triage`, `later`, `decisions`, and `history`.
`harrow --view next` reads the same selection. Search with `--all` before
filing a proposal, because closed and dropped items hold previous decisions.
The `decisions` view selects explicitly typed decisions; older reasoning also
lives in ordinary feature and documentation items.

After any two new features, the next investment is observing a real user's
workflow before selecting a third. The author's daily use counts as evidence;
0066 adds outside perspectives. Correctness fixes continue when needed.
Record the observation and any changed priorities in items, not a second
planning document.

## Before you start

Set up the local Git integration once after cloning:

```sh
cairn init --git
```

Claim the item so other writers in the same working copy can see it is taken:

```sh
cairn claim 23
```

That writes your name into the item and moves it to an active status. If you
change your mind, `cairn release 23` hands it back.

For agents, set `CAIRN_AGENT` to the agent's name and use `CAIRN_USER` or
`claim --as` for an explicit assignee. An `owner` is the person accountable
for an external decision; it is not a second claim.

Claims and locks are local to the item directory. When work is split across
branches, linked worktrees, or clones, coordinate the assignments before
splitting. Commit the item and code together, review both, and run `cairn check`
after merging. Do not infer distributed exclusion from a successful claim.

## While you work

Record what you learn in the item, not in a scratch file:

```sh
cairn set 23 labels+=needs-windows-testing
cairn note 23 "Reproduced on Windows; checking the rename path."
cairn edit 23        # opens $EDITOR on the item itself
```

## Before you send it

```sh
make check     # fmt, clippy, tests, and cairn's own roadmap
make audit     # advisories, licences and sources (needs cargo-deny)
```

That runs `cargo fmt --check`, `cargo clippy -- -D warnings`, the full test
suite, and `cairn check --render --strict` over cairn's own roadmap. All of it
must pass. If you changed behaviour that the README demo shows, run `make demo`
and commit the regenerated SVG.

Tick only criteria that have become true, then `cairn close 23`. Run
`cairn check --render --strict` again after closing and include the item and
rendered roadmap in the pull request. Old closed items with unticked boxes
are not evidence that those checks passed; do not tick them retroactively to
make a progress count look better.

After a schema change, run `cairn agent --write AGENTS.md` and `cairn render`.
The project-specific guidance outside the generated markers is preserved.

## Releasing

`doc/RELEASING.md` is the checklist. Most of it is automated; the parts that are
not are decisions somebody has to make, and they are written down so they are
not remembered instead.

## Reporting a bug

Open a GitHub issue. You are not expected to learn cairn to report a problem
with it — write it however is natural, and a maintainer will bring it into the
backlog with `cairn import --from github`.

## Four rules

These are short because each of them was learned the expensive way. The three
that a test can hold to account are held by `tests/rules.rs`, so that they
cannot be quietly undone; the fourth is a judgment, and lives here because
judgments are what review is for.

### A new command has to earn its line

> A new command earns its place if removing it would make a real task *harder*,
> not merely different. If an existing command with a flag would do, that is the
> answer.

The command surface is already substantial. Every addition creates another
piece of documentation, compatibility, and testing to maintain. Apply the bar
before writing code; a common workflow should remain learnable through a small
set of commands even when the reference interface is broad.

> A command that exists to be tested rather than used is hidden from `--help`.

`merge-driver` is called by Git, so it is hidden and documented in the manual.
`migrate` is visible: older project formats exist and people need to discover
how to update them. `tests/rules.rs` holds that distinction to account.

### Prefer schema over a documented key

> A documented key is a promise to everyone who will ever write a cairn file.
> Schema is a promise to one project. Reach for the second first.

Format 2 moved milestones out of `cairn.toml` and into items. It touched **zero
item files** across seven real projects and three hundred items: the whole cost
was one configuration file per project and a command nobody had to think hard
about. What moved was schema, which is one project's business. What did not move
was the format, which is everybody's.

That is the same instinct that made status *categories* fixed while status
*names* are chosen — a small permanent axis underneath, everything expressive
above it.

So the answer to "cairn should have a field for X" is usually a `[[field]]`
block. A pull request that adds a key to §4 of the specification has to say why
schema would not do. What it costs if you get it wrong is in the manual, under
"What would still cost a format number".

### The specification changes first

> A change to what cairn writes to disk edits `spec/README.md` first, in the
> same pull request, before the code.

A specification written after the code is a description. Written before it, it
is a design tool — and it has already earned that: writing down that `id` is an
unsigned integer is what made a later identifier design obviously wrong, by
ruling out the answer that looked right.

Two things follow from doing it in that order. Somebody has to decide whether
the change is additive, and therefore free under the compatibility rules,
*before* building it rather than after. And the pull request shows the intended
contract next to the code implementing it, which is where a reviewer can
disagree with it cheaply.

### There is never a library target

> `Cargo.toml` has a `[[bin]]` and no `[lib]`, and that is a decision.

This rule used to rest on the licence: linking cairn made you a derivative work
and inherited the GPL, reading its documented format did not, and pointing
people at the format was how they avoided that. Under MIT the legal half of that
argument is gone — linking cairn now costs you nothing.

The rule stays, and the honest reason is a different one.

**A Rust API would be a second contract, and a worse one.** The specification is
this project's strongest asset: it is versioned, it has a conformance corpus, a
second independent reader checks it, and every format that has ever existed is
still tested against the current build. A `[lib]` would be a parallel promise
with none of that — semver on every internal type, breakage on every refactor,
and a standing temptation to keep a bad shape because something depends on it.

**And a linkable cairn makes the specification decorative.** If the easy path is
`use cairn::…`, nobody reads the format, the format stops being exercised by
anything but cairn's own tests, and the thing that lets this outlive the
implementation quietly rots.

So when a second program wants to read items, the answer is the format, or the
binary's JSON output and exit codes. Both are documented under "Integrating" in
the manual. Adding a `[lib]` fails the build.

## What the tests are for

### The harness

Every integration test shares `tests/support/`. There used to be four
`Project`s, three random generators and two `Out`s across six files, and they
had drifted — `run` returned three different types depending on which file you
were in — because an improvement to the harness had to be made four times or not
at all.

Two things in it are worth knowing before writing a test.

**Build a schema; do not edit one.** `Schema` assembles `cairn.toml` from
values:

```rust
let p = Project::with(
    Schema::standard()
        .field(Field::text("risk").agent(Agent::ReadOnly))
        .amend_status("done", |s| s.agent(Agent::Propose))
        .render(|r| r.group_by("epic").link_items()),
);
```

This replaced forty-one `.replace()` calls against the shipped template, which
broke constantly: a `[render]` table appended twice, a `link_items = false` that
had moved, a `title = "Roadmap"` that was not where the test guessed. A schema
built from parts cannot be wrong about the file it is editing, because it is not
editing one. It refuses a duplicate field or status at the line that added it,
rather than leaving cairn to complain later, and `Schema::standard()` is itself
tested against the real program.

`Project::new()` still writes the shipped template, for the tests that are about
the template.

**The assertions print the difference, not the haystack.** `assert_contains`
shows the nearest matching lines and an excerpt; `assert_json` names a dotted
path; `assert_lines_eq` prints only the lines that differ. A failure that dumps
five kilobytes of JSON is a failure nobody reads.

### What each level proves

`make check` is what every pull request must pass: the suite, formatting, lints,
and cairn's own backlog validated against its own schema.

It does **not** exercise concurrency. Both soak tests are `#[ignore]`, so
`cargo test` reports them as ignored and the lock, the atomic write and
identifier allocation get no contention in the ordinary loop. The fuzz pass it
runs is 600 argument vectors; the long one is 20,000.

That is a deliberate trade — a suite nobody will wait for is a suite nobody runs
— and the cost of it is that "check passes" carries more weight in the head than
it has earned. So the rest has a name:

```
make durability     # check, then soak, then the long fuzz, then conformance
make coverage       # line and region coverage, with a floor
```

Run it before a release, and after touching any of four things: **the lock, the
write path, identifier allocation, or the merge driver.** Those are where a
defect costs somebody their work rather than their afternoon. Every stage prints
the seed it used, so a failure is reproducible.

The suite is not decoration. It encodes decisions that are easy to undo by
accident:

The end-to-end tests are split by subject rather than kept in one file:
`basics`, `schema`, `workflow`, `agents`, `repository`, `format`,
`presentation`, `durability`. Somebody looking for how milestones are tested
should not have to scroll eight thousand lines to find out.

- `tests/golden/` pins how item files parse. Changing an expectation there is a
  format change and needs a format number and a migration. See "Compatibility"
  in the manual.
- `tests/golden/format-N/` is that format's corpus, frozen the day it stopped
  being current, and a digest is asserted so it cannot be quietly edited. If a
  frozen case looks wrong, it was wrong then — add a case to the current corpus
  instead. Bumping the format means freezing the outgoing one in the same
  change; the build fails otherwise.
- `tests/rules.rs` guards what of the four rules a test can guard — no library
  target, no command that reaches the network, no hidden command, and a
  specification that never defers to this implementation — and the promise.
- The concurrency tests race real processes. If you touch the lock, the write
  path, or identifier allocation, they are the ones that matter.
- The property tests generate adversarial titles and bodies. If you touch the
  parser, expect them to find something.

### Where to spend a new test

Worth knowing before you write one, because it is not what you would guess.
**Close to none of the defects found in cairn so far came from example tests** —
the kind written in advance, describing a situation somebody had already thought
of. They came from the soak test, from a platform nobody here can run, from an
adversarial reader, and from attempting a release.

Example tests are still the regression net, and every defect found another way
leaves one behind. But if you are adding coverage rather than pinning a fix,
these have the better return:

```sh
make soak                                        # a long arbitrary sequence
CAIRN_SOAK_SEED=7 CAIRN_SOAK_OPS=2000 make soak  # reproduce or go deeper
cargo test --release --test fuzz_args            # arbitrary argument vectors
```

And if the thing you are testing is true of *every* input rather than of one
you chose, state it as a property. `tests/interchange.rs` and
`tests/filter_laws.rs` are the shape to copy: the first found that `import` was
silently dropping every dependency, which nothing failed on and `check` was
perfectly happy with.

## Style

Match the surrounding code. Comments explain *why* a thing is the way it is —
the constraint, the trade-off, the failure it prevents — not what the next line
does. If a decision cost you an hour to reach, write down the reason so nobody
spends that hour again.

## Copyright, and the agreement

cairn is distributed under the MIT licence.

**You keep the copyright in everything you write.** Contributing does not
transfer it, and nothing stops you using your own work however you like,
including elsewhere.

Add yourself to `AUTHORS` with your first accepted change.

### Why there is no agreement to sign

There used to be one, and the reason was the GPL. A copyleft project that might
one day want to offer other terms has to be able to relicense, which means every
contributor must have granted permission in advance — otherwise changing terms
means finding everybody and asking, and one refusal or one unreachable person
settles it for good.

Under MIT that problem does not arise. The licence already permits everything a
relicence would have been for: anybody may use, modify, sublicense and sell,
including inside proprietary software. There is nothing left for an agreement to
unlock, so asking you to sign one would be asking for a signature that buys
nobody anything.

What is asked instead is the ordinary thing: that you have the right to
contribute what you are contributing, and that you are content for it to go out
under MIT along with the rest.

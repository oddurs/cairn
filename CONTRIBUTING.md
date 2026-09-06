# Contributing to cairn

cairn tracks its own roadmap, so the backlog is the contribution guide.

```sh
cargo build --release
./target/release/cairn next          # what is ready to work on
./target/release/cairn next --blocked   # and what is waiting on something
./target/release/cairn show 23       # the reasoning behind an item
```

Items carry the thinking that produced them — the problem, the proposal, the
costs that were weighed, and acceptance criteria you can check yourself. If an
item's body does not tell you enough to start, that is a bug in the item; say so.

## Before you start

Claim the item, so nobody duplicates your work:

```sh
cairn claim 23
```

That writes your name into the item and moves it to an active status. If you
change your mind, `cairn release 23` hands it back.

## While you work

Record what you learn in the item, not in a scratch file:

```sh
cairn set 23 labels+=needs-windows-testing
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

Then `cairn close 23` and open a pull request.

## Reporting a bug

Open a GitHub issue. You are not expected to learn cairn to report a problem
with it — write it however is natural, and a maintainer will bring it into the
backlog with `cairn import --from github`.

## Three rules

These are short because each of them was learned the expensive way, and each is
enforced by a test in `tests/rules.rs` so that it cannot be quietly undone.

### A new command has to earn its line

> A new command earns its place if removing it would make a real task *harder*,
> not merely different. If an existing command with a flag would do, that is the
> answer.

cairn has twenty-eight commands, written over three days, each locally
justified and collectively more than anybody needs. There was never a bar;
every one of them seemed reasonable at the moment it was written, which is how
surfaces grow. Applying the bar before the code is written is much cheaper than
pruning afterwards.

> A command that exists to be tested rather than used is hidden from `--help`.

`merge-driver` is called by git, not by people. `migrate` exists so that the
migration path is exercised long before it is needed — a good reason for the
command to exist and a poor reason for it to occupy a line in the help output.
Both are hidden and both are documented in the manual: hidden is not the same
as undocumented. When there is genuinely something to migrate, `migrate` stops
being a command that exists to be tested, and the test in `tests/rules.rs` will
tell you to unhide it.

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

A program that links cairn is a derivative work and inherits the GPL. A program
that reads cairn's documented file format is not. That difference is the entire
reason the specification exists — it is what lets other people build on cairn's
conventions without adopting cairn's licence, and the specification is this
project's strongest asset.

So when a second program wants to read items, the answer is the format, or the
binary's JSON output and exit codes. Both are documented under "Integrating" in
the manual. Adding a `[lib]` fails the build.

## What the tests are for

The suite is not decoration. It encodes decisions that are easy to undo by
accident:

- `tests/golden/` pins how item files parse. Changing an expectation there is a
  format change and needs a format number and a migration. See "Compatibility"
  in the manual.
- The concurrency tests race real processes. If you touch the lock, the write
  path, or identifier allocation, they are the ones that matter.
- The property tests generate adversarial titles and bodies. If you touch the
  parser, expect them to find something.

## Style

Match the surrounding code. Comments explain *why* a thing is the way it is —
the constraint, the trade-off, the failure it prevents — not what the next line
does. If a decision cost you an hour to reach, write down the reason so nobody
spends that hour again.

## Copyright, and the agreement

cairn is distributed under the GNU General Public Licence, version 3 or later,
and that is not changing.

Before your first pull request is merged you will be asked to agree to a
[contributor licence agreement](CLA.md). It is short. **You keep the copyright
in everything you write** — it is a licence, not an assignment, and nothing in
it stops you using your own work however you like, including elsewhere.

Add yourself to `AUTHORS` with your first accepted change.

### Why there is an agreement

Because you are entitled to know what you are signing.

Today the maintainer is the sole copyright holder, and can therefore offer cairn
under terms other than the GPL — for instance a commercial licence alongside it,
if a hosted version for teams ever makes sense. The moment a contribution lands
without an agreement in place, that stops being possible: changing terms would
require finding every contributor and asking permission, and one refusal or one
unreachable person settles it for good.

The agreement keeps that option open. It does not commit anyone to exercising
it, and it does not affect the GPL you receive cairn under: that licence, once
given, cannot be withdrawn from you or from anybody else.

If you would rather not sign, say so on the pull request. Small fixes can often
be reimplemented independently, and a bug report costs you nothing and is worth
a great deal.

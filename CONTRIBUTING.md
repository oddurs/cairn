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

## Copyright

cairn is GPL-3.0-or-later. By contributing you agree your work is licensed the
same way. There is no copyright assignment. Add yourself to `AUTHORS` with your
first accepted change.

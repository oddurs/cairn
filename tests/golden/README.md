# Golden corpus

Item files that must keep parsing to the same values forever.

Each `NAME.md` is an item; `NAME.json` records what cairn must read out of it.
The test in `tests/cli.rs` copies each file into a fresh project and compares
`cairn show --json` against the expectation, ignoring only the file's own path.

The point is not to pin down today's behaviour but to make a change in
interpretation impossible to make by accident. Every format bug found should
arrive here as a new case. Editing an expectation is a deliberate act: it means
the on-disk format changed, which needs a format number and a migration.

The corpus deliberately includes files cairn would not write itself — bare
strings where a list is expected, missing ids, CRLF endings, unknown keys —
because those are what people, editors, and other tools produce.

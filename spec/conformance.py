#!/usr/bin/env python3
"""Run the reference reader against the golden corpus.

The corpus in `tests/golden` pins how cairn parses item files, including files
cairn would never write itself. Running an independent reader over the same
files and comparing answers is what turns "a reader can be written from the
specification" from a claim into a fact — and when the two disagree, one of them
is wrong, which is the useful part.

    python3 spec/conformance.py            # from the repository root

Two keys in the expectations are skipped, and the reason is worth stating rather
than hiding in a filter.

`category` is a property of the *project*: §7 says a status belongs to a
category, and the mapping lives in the project's configuration. The corpus has
no configuration, so cairn's answers there come from its default schema. An
item-level reader cannot know them and should not pretend to.

`ref` is likewise project-level — §4.2, how the project renders identifiers.

Everything else is item content, and must agree exactly.

Copyright (C) 2026 Oddur Sigurdsson. Permissive; see reader.py.
"""

import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import reader  # noqa: E402

# Derived from the project, not from the item. See the module docstring.
PROJECT_LEVEL = {"category", "ref"}


def main():
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    corpus = os.path.join(root, "tests", "golden")
    if not os.path.isdir(corpus):
        print(f"no corpus at {corpus}", file=sys.stderr)
        return 2

    cases = sorted(f for f in os.listdir(corpus) if f.endswith(".md") and f != "README.md")
    if not cases:
        print("the corpus is empty, so this test asserts nothing", file=sys.stderr)
        return 2

    failures = 0
    for case in cases:
        path = os.path.join(corpus, case)
        expected_path = path[:-3] + ".json"
        if not os.path.exists(expected_path):
            print(f"  ?  {case}: no expectation committed")
            continue

        with open(expected_path, encoding="utf-8") as f:
            expected = {k: v for k, v in json.load(f).items() if k not in PROJECT_LEVEL}

        try:
            got = reader.read(path)
        except reader.NotAnItem as e:
            print(f"  ✗  {case}: the reference reader rejected it: {e}")
            failures += 1
            continue

        got = {k: v for k, v in got.items() if k not in PROJECT_LEVEL}
        if got == expected:
            print(f"  ✓  {case}")
            continue

        failures += 1
        print(f"  ✗  {case}")
        for key in sorted(set(expected) | set(got)):
            if expected.get(key) != got.get(key):
                print(f"       {key}:")
                print(f"         cairn expects: {expected.get(key)!r}")
                print(f"         this reader:   {got.get(key)!r}")

    print()
    if failures:
        print(
            f"{failures} of {len(cases)} disagree.\n"
            "One of the two is wrong. Resolve it by changing the specification "
            "or the corpus — never by teaching this reader what cairn happens "
            "to do, which would make it agree without making it correct."
        )
        return 1

    print(f"{len(cases)} cases, two independent readers, same answers.")
    return 0


if __name__ == "__main__":
    sys.exit(main())

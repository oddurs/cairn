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
try:
    import reader  # noqa: E402
except ModuleNotFoundError as missing:
    # The reader needs a YAML parser, because §6 says an unquoted scalar is
    # resolved by YAML rules and writing a second YAML implementation to check
    # the first one would prove nothing. Say what to install rather than
    # printing a traceback at somebody running `make durability`.
    # The module is `yaml`; the thing to install is `pyyaml`.
    package = {"yaml": "pyyaml"}.get(missing.name, missing.name or "it")
    print(
        f"the reference reader needs the `{missing.name}` module:\n"
        f"    python3 -m pip install {package}\n"
        "or run it inside a virtual environment that has it.",
        file=sys.stderr,
    )
    sys.exit(2)

# Derived from the project, not from the item. See the module docstring.
PROJECT_LEVEL = {"category", "ref"}


def corpora(root):
    """The current corpus, and one for every format that has ever existed.

    A specification that only describes the newest format is not a
    specification of the format; it is a specification of the moment. A reader
    written from it has to read what people actually wrote.
    """
    base = os.path.join(root, "tests", "golden")
    yield ("current", base)
    for name in sorted(os.listdir(base)):
        path = os.path.join(base, name)
        if name.startswith("format-") and os.path.isdir(path):
            yield (name, path)


def main():
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    base = os.path.join(root, "tests", "golden")
    if not os.path.isdir(base):
        print(f"no corpus at {base}", file=sys.stderr)
        return 2

    failures = 0
    checked = 0

    for label, corpus in corpora(root):
        print(f"{label}:")
        failures += run(corpus, frozen=label != "current")
        checked += len(
            [f for f in os.listdir(corpus) if f.endswith(".md") and f != "README.md"]
        )

    print()
    if failures:
        print(
            f"{failures} disagree.\n"
            "One of the two is wrong. Resolve it by changing the specification "
            "or the corpus — never by teaching this reader what cairn happens "
            "to do, which would make it agree without making it correct."
        )
        return 1

    print(f"{checked} cases across every format, two independent readers, same answers.")
    return 0


def run(corpus, frozen=False):
    cases = sorted(f for f in os.listdir(corpus) if f.endswith(".md") and f != "README.md")
    if not cases:
        print("  the corpus is empty, so this test asserts nothing", file=sys.stderr)
        return 1

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
        if frozen:
            # An expectation frozen at format N names every key that existed at
            # format N. A later format may add keys; the older reading is still
            # correct, so compare on what the expectation actually claims.
            got = {k: v for k, v in got.items() if k in expected}
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

    return failures


if __name__ == "__main__":
    sys.exit(main())

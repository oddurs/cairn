#!/usr/bin/env python3
"""A reader for the cairn item format, written from the specification.

This is a second implementation. It exists because `spec/README.md` claims a
reader can be written from it alone, and until somebody did that, the claim was
a hope: every specification that has never been implemented twice contains at
least one thing that is only true because of how the original happens to work.

It was written from the specification and not from cairn's source. Reading the
Rust would have defeated the purpose entirely.

Deliberately short, so that reading it is a reasonable way to learn the format.
`spec/conformance.py` runs it against the corpus in `tests/golden` and compares
its answers to the committed expectations; when they disagree, one of them is
wrong, and finding out which is the point.

Copyright (C) 2026 Oddur Sigurdsson.

Permission to use, copy, modify, and distribute this software for any purpose
with or without fee is hereby granted, provided that the above copyright notice
and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH
REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND
FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT,
INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS
OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER
TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF
THIS SOFTWARE.

(This file is permissive, unlike cairn itself, which is GPL. The format is meant
to be built on — including by proprietary tools — and a GPL reference reader
would discourage precisely the thing it exists to encourage.)
"""

import json
import os
import re
import sys

import yaml  # §6: YAML resolves unquoted scalars, so a YAML parser is required.


class Core(yaml.SafeLoader):
    """A loader that resolves scalars by the YAML 1.2 core schema, as §6 requires.

    PyYAML implements YAML 1.1, which disagrees with 1.2 about exactly the
    values people write by hand: `no` is a boolean in 1.1 and the string "no" in
    1.2, and `12:30` is the integer 750 in 1.1 and the string "12:30" in 1.2.

    This is not a detail. Running this reader against cairn's corpus with
    PyYAML's defaults is what found the specification saying "YAML's own rules"
    without saying which YAML — under which its own three examples were
    ambiguous. Two conforming readers could have reported different content for
    the same file.
    """


Core.yaml_implicit_resolvers = {}
for _tag, _pattern, _first in [
    ("tag:yaml.org,2002:null", r"^(?:~|null|Null|NULL|)$", "~nN\0"),
    ("tag:yaml.org,2002:bool", r"^(?:true|True|TRUE|false|False|FALSE)$", "tTfF"),
    (
        "tag:yaml.org,2002:int",
        r"^(?:[-+]?[0-9]+|0o[0-7]+|0x[0-9a-fA-F]+)$",
        "-+0123456789",
    ),
    (
        "tag:yaml.org,2002:float",
        r"^(?:[-+]?(?:\.[0-9]+|[0-9]+(?:\.[0-9]*)?)(?:[eE][-+]?[0-9]+)?"
        r"|[-+]?\.(?:inf|Inf|INF)|\.(?:nan|NaN|NAN))$",
        "-+0123456789.",
    ),
]:
    Core.add_implicit_resolver(_tag, __import__("re").compile(_pattern), list(_first))

# §4. Every key the specification names. Anything else is a custom field.
KNOWN = (
    "id",
    "key",
    "title",
    "type",
    "status",
    "milestone",
    "assignee",
    "owner",
    "created_by",
    "labels",
    "depends_on",
    "created",
    "updated",
    "source",
)


class NotAnItem(Exception):
    """§3: a file without both delimiters is not an item and must be rejected."""


def split(text):
    """Return (frontmatter, body). §3."""
    text = text.lstrip("﻿")  # §3.1: an optional byte order mark.
    text = text.replace("\r\n", "\n")  # §3: LF or CRLF, both accepted.

    if not text.startswith("---"):
        raise NotAnItem("no opening delimiter")
    after = text[3:]
    if not after.startswith("\n"):
        # The opening delimiter is a line consisting of exactly `---`.
        if after.strip("\r\n \t") != "" or "\n" not in after:
            raise NotAnItem("no opening delimiter")
    nl = after.find("\n")
    if nl == -1:
        raise NotAnItem("no closing delimiter")
    rest = after[nl + 1 :]

    offset = 0
    for line in rest.splitlines(keepends=True):
        # §3: the *first* line that trims to `---` or `...` ends the
        # frontmatter. Later ones are body text.
        if line.rstrip() in ("---", "..."):
            return rest[:offset], rest[offset + len(line) :]
        offset += len(line)
    raise NotAnItem("no closing delimiter")


def as_list(value):
    """§4: a sequence, or a single string split on commas."""
    if value is None:
        return []
    if isinstance(value, list):
        return [str(v) for v in value]
    return [part.strip() for part in str(value).split(",") if part.strip()]


def as_ids(value):
    """§4: like as_list, but integers, each with an optional leading `#`."""
    return [int(str(v).lstrip("#").strip()) for v in as_list(value)]


def id_from_filename(path):
    """§4.2: a leading run of digits, when `id` is absent."""
    stem = os.path.basename(path)
    stem = stem[:-3] if stem.endswith(".md") else stem
    digits = re.match(r"\d+", stem)
    return int(digits.group()) if digits else None


def read(path, text=None):
    """Parse one item file into a dictionary of its documented keys."""
    if text is None:
        with open(path, "rb") as f:
            text = f.read().decode("utf-8")

    front, body = split(text)
    meta = yaml.load(front, Loader=Core) if front.strip() else {}
    if meta is None:
        meta = {}
    if not isinstance(meta, dict):
        raise NotAnItem("frontmatter is not a mapping")

    ident = meta.get("id")
    if ident is None:
        ident = id_from_filename(path)
    if ident is None:
        raise NotAnItem("no `id` and no leading digits in the filename")

    def text_or_none(key):
        value = meta.get(key)
        return None if value is None else str(value)

    return {
        "id": int(ident),
        "key": text_or_none("key"),
        "title": text_or_none("title"),
        "type": text_or_none("type"),
        "status": text_or_none("status"),
        "milestone": text_or_none("milestone"),
        "assignee": text_or_none("assignee"),
        "owner": text_or_none("owner"),
        "created_by": text_or_none("created_by"),
        "labels": as_list(meta.get("labels")),
        "depends_on": as_ids(meta.get("depends_on")),
        "created": text_or_none("created"),
        "updated": text_or_none("updated"),
        "source": text_or_none("source"),
        # §4: anything else is a custom field, and a reader preserves it.
        "fields": {k: v for k, v in meta.items() if k not in KNOWN},
        # §3: the body is arbitrary text and is never interpreted. Its leading
        # blank line is the separator, not content.
        "body": body.lstrip("\n"),
    }


def main(argv):
    if len(argv) != 2:
        print("usage: reader.py ITEM.md", file=sys.stderr)
        return 2
    try:
        print(json.dumps(read(argv[1]), indent=2, sort_keys=True, ensure_ascii=False))
    except NotAnItem as e:
        print(f"{argv[1]}: not an item: {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))

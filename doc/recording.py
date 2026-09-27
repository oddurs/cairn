"""Fixed tags for reproducible recordings of real command output.

Every item cairn creates gets a random `uid`, and a recording that changed on
every run would be noise in every diff. The program has no seed or special
identity mode, and should not: a tag is a line in a file, the file is the
source of truth, and rewriting it is what anybody may do.
"""
import os
import re


def identity(n):
    return f"{n:08x}-0000-4000-8000-{n:012x}"


def pin_tags(work):
    """Give each item the tag its number implies."""
    items = os.path.join(work, "cairn", "items")
    for name in sorted(os.listdir(items)):
        if not name.endswith(".md"):
            continue
        path = os.path.join(items, name)
        with open(path, encoding="utf-8") as f:
            text = f.read()
        number = re.search(r"^id: (\d+)$", text, re.M)
        if number:
            text = re.sub(r"^uid: .*$", f"uid: {identity(int(number.group(1)))}",
                          text, count=1, flags=re.M)
            with open(path, "w", encoding="utf-8") as f:
                f.write(text)

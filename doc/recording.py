"""Fixed native identities for reproducible recordings of real command output.

Importing an existing record preserves its UUID. Production creation still uses
OS randomness; there is no seed or special identity mode in the program.
"""
import json
import subprocess


def identity(n):
    return f"{n:08x}-0000-4000-8000-{n:012x}"


def populate(cairn, work, env, extended=False):
    items = [
        {"title": "First usable version", "type": "milestone", "key": "v0.1",
         "fields": {"due": "2026-12-01"}},
        {"title": "Hardening", "type": "milestone", "key": "v0.2",
         "fields": {"due": "2027-02-01"}},
        {"title": "Support OAuth login", "type": "feature", "milestone": "v0.1",
         "status": "doing" if extended else "backlog", "fields": {"priority": "p0"}},
        {"title": "Rate-limit the public API", "type": "feature", "milestone": "v0.2",
         "depends_on": [identity(3)], "fields": {"priority": "p1"}},
    ]
    if extended:
        items.extend([
            {"title": "Board shears on narrow terminals", "type": "bug",
             "milestone": "v0.1", "status": "planned", "fields": {"priority": "p1"}},
            {"title": "Document the export format", "type": "docs",
             "milestone": "v0.2", "fields": {"priority": "p2"}},
        ])
    for n, item in enumerate(items, 1):
        item.update(id=identity(n), created="2026-09-05", updated="2026-09-05")
    subprocess.run([cairn, "import", "-"], cwd=work, env=env,
                   input=json.dumps({"cairn": "2", "items": items}),
                   text=True, capture_output=True, check=True)

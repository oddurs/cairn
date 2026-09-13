---
id: 118
title: Unquoted YAML 1.1 booleans disagree across readers
type: bug
status: backlog
milestone: v1.0
created: 2026-09-12
updated: 2026-09-12
priority: p2
area: format
---

## What happens

serde_yaml_ng writes a string field unquoted when YAML 1.2 would read it back as
a string. YAML 1.1 — which PyYAML implements, and `spec/reader.py` uses — reads
four of those spellings as booleans.

```
title: no          # PyYAML: False
assignee: on       # PyYAML: True
key: yes           # PyYAML: True
area: off          # PyYAML: False
```

So two conforming readers disagree about the same file, which is the exact
failure `spec/conformance.py` exists to catch. It does not catch it today
because no corpus case carries one of these values.

## What should happen

A scalar that any reader in common use would take for a non-string is quoted on
write, the way `1.0`, `true` and `null` already are. serde_yaml_ng quotes for
YAML 1.2 only; the set needs widening to the YAML 1.1 booleans.

## Reproduction

1. `cairn init --bare`
2. `cairn new "no"`
3. `cairn set 1 assignee=on`
4. `grep -E "^(title|assignee):" cairn/items/0001-no.md` — both unquoted

## Notes

Predates 0.2.0; confirmed against a stock 0.2.0 build, not introduced by any
recent change. Found while checking what keys `cairn new -t milestone` can
derive from a title, since a milestone titled "No" or "On" reaches it by an
ordinary route.

Worth a corpus case either way: a golden file carrying `key: no` would have made
the two readers disagree out loud.

## Acceptance criteria

- [ ] `yes`, `no`, `on`, `off` and their capitalisations are quoted on write
- [ ] A golden corpus case carries one, and both readers agree about it
- [ ] Existing items are not rewritten merely by being read

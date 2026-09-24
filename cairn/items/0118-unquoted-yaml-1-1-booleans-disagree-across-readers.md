---
id: fc533731-5814-47bf-9254-7f081aba16f5
title: Unquoted YAML 1.1 booleans disagree across readers
type: bug
status: dropped
milestone: v1.0
created: 2026-09-12
updated: 2026-09-17
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

## 2026-09-17

Not a defect. Withdrawn after measuring it properly.

The original report tested with `yaml.safe_load`, which is PyYAML's default and implements YAML 1.1. That is not what this project uses. Spec §6 requires the **1.2 core schema**, and `spec/reader.py` implements it with a custom `Core` loader that strips PyYAML's 1.1 resolvers and reinstates the 1.2 set — the spec even says so in the paragraph the report should have read. Under 1.2 core, `no`, `yes`, `on`, `off` and `12:30` are strings. Both readers agree, and there is no disagreement for `conformance.py` to catch.

§6 also puts a **must** on writers: quote any value that would otherwise change meaning when read back. Measured across 34 hazardous scalars through a text field, a list field and the title: cairn quotes all 29 that resolve to a non-string under 1.2 core — `0x1F`, `1e5`, `.inf`, `~`, `null`, `true`, `0`, `1.0`, `-`, `[]`, `#hash`, `a: b`, `*anchor`, `%dir` and the rest. It writes exactly five bare, and all five are the 1.1-only set above, where bare *is* correct under the required schema.

So cairn is conformant on both sides, and the residual hazard is bounded to five value shapes read by a non-conforming reader — which §6 already names, and for which the documented remedy is to quote by hand.

Not changing the writer. serde_yaml_ng decides plain-versus-quoted itself, so forcing those five would need a custom emitter or post-processing the YAML text, and neither buys any conformance.

What the investigation did leave behind: nothing had ever tested the §6 writer requirement. The corpus covers reading, and `tests/golden/yaml-scalars.md` even asserts 'cairn quotes anything it writes' in prose with no test behind it. `a_value_that_would_change_meaning_is_quoted_on_the_way_out` in tests/format.rs now asserts it, and was verified to fail when a plain value is added to the must-quote list.

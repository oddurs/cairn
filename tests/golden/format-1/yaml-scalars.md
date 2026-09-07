---
id: 11
title: Scalars YAML resolves before cairn sees them
status: backlog
assignee: no
labels: [12:30, 0x1F, 1.20]
---

Unquoted values that look like numbers, booleans or times are resolved by YAML
itself. cairn quotes anything it writes, so this only affects hand-written
files; quote such values to keep them verbatim.

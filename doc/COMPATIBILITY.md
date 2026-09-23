# The Cairn–Harrow contract

Cairn is the writer and command interface; Harrow is the independent reader and
human interface. They share a documented format and behavior tests, not a Rust
library, database, service or process on every read.

Cairn 0.3 uses **format 3**, unchanged. It tests its current and frozen format-1
and format-2 corpora, plus the pinned Harrow revision in
[`spec/harrow-revision`](../spec/harrow-revision). That revision is the minimum
verified companion for completion-date queries and the expanded query contract.
The older Harrow 0.1.0 release reads format 3 but is not the verified pair: it
rejects `closed_at` filters and disagrees on some derived fields. The tested
revision is available on Harrow main through PR 93; do not infer compatibility
from the still-unchanged Harrow package version alone.

```sh
make conformance
make agreement HARROW_REPO=/path/to/harrow
```

The agreement target requires that exact counterpart revision, refuses modified
contract code, and explicitly runs the cross-tool tests. Missing tools, fixtures,
or disagreements fail. CI checks out the pin and runs the same target on every
PR and main push. Harrow's required CI reciprocally tests a pinned Cairn release.

The gate compares machine-readable item sets for all eight views in Cairn's
own project and semantic fixtures for dates, categories, dependency readiness,
criteria, hierarchy and filter operators. The ordinary Harrow suite reads the
vendored 36-case corpus; its PROVENANCE identifies the upstream commit. To
advance the pin, review both projects' contract changes, refresh upstream
fixtures unmodified, run both suites, and commit the new pin deliberately.

## Differences that are intentional

Harrow adds interactive free-text filtering; Cairn uses `search`. Harrow rejects
unknown fields while Cairn warns on queries and validates with `check`. Grouped
TUI headings are not item rows: compare ungrouped IDs, not presentation order.
`next` ranks active work first and excludes containers and dependency-blocked
items in addition to a saved view's selection. A view is not an authorization
boundary; explicit assignments remain possible.

Missing values compare as empty strings in range predicates. Use
`closed_at!=,closed_at<2026-09-10` for dated history before a cutoff.
`criteria_met=true` includes items with no criteria; add `criteria>0` if that
distinction matters. `ready` includes unfinished active work and says nothing
about approval or assignment.

Harrow statistics prefer recorded `closed_at`; legacy undated records retain
an `updated` estimate. Date filters do not invent completion dates. Harrow may
open a future format read-only for recovery; that is not verified compatibility.

Report mismatches with both tool versions and revisions, the selected view or
filter, schema, and a small non-sensitive fixture. Do not change a fixture's
expected answer merely to make the readers agree.

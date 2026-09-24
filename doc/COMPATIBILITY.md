# The Cairn–Harrow contract

Cairn is the writer and command interface; Harrow is the independent reader and
human interface. They share a documented format and behavior tests, not a Rust
library, database, service or process on every read.

The 1.0.0-alpha.1 development line writes **format 4**, with immutable UUIDv4
identities. The paired Harrow 0.2.0-alpha.1 development line reads formats 1–4.
Neither alpha is a declaration that the broader 1.0 milestone is finished.
Cairn 0.3.0 and Harrow 0.1.0 cannot read format 4: upgrade both before migrating.
Exact tested revisions, not package version alone, establish the pair; Cairn
pins Harrow in [`spec/harrow-revision`](../spec/harrow-revision).

Files, JSON, hooks, dependencies and declared ID references carry full UUIDs.
Human commands accept unique prefixes of at least eight hex digits; these
prefixes are display handles, never persisted identities. Migration freezes old
numbers in `_legacy-ids.toml`, preserves filenames and body bytes, and bridges
historical lookup without rewriting Git. Commit one migration and merge it to
other branches; do not independently migrate copies of the same backlog.

```sh
make conformance
make agreement HARROW_REPO=/path/to/harrow
```

The agreement target requires that exact counterpart revision, refuses modified
contract code, and explicitly runs the cross-tool tests. Missing tools, fixtures,
or disagreements fail. CI checks out the pin and runs the same target on every
PR and main push. Harrow's required CI reciprocally tests a pinned Cairn revision.

The gate compares machine-readable item sets for all eight views in Cairn's
own project and semantic fixtures for dates, categories, dependency readiness,
criteria, hierarchy, filter operators, UUIDs, prefixes and migrated aliases.
The ordinary Harrow suite reads the vendored 49-case corpus across formats 1–4;
its PROVENANCE identifies the upstream commit. To
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
an `updated` estimate. Date filters do not invent completion dates. Harrow refuses
unknown future formats and an unfinished identity migration rather than reading
them with the wrong identity semantics.

Report mismatches with both tool versions and revisions, the selected view or
filter, schema, and a small non-sensitive fixture. Do not change a fixture's
expected answer merely to make the readers agree.

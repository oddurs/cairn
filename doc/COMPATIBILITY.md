# The Cairn–Harrow contract

Cairn is the writer and command interface; Harrow is the independent reader and
human interface. They share a documented format and behavior tests, not a Rust
library, database, service or process on every read.

The 1.0.0-alpha.1 development line writes **format 5**: numbered items, each
with an optional `uid` tag, and per-type id renderings. It reads formats 1–4
and writes none of them. The pinned Harrow reads formats 1–4 and **not yet
5**: do not migrate a project Harrow watches until a Harrow that reads format 5
is pinned here. Neither alpha is a declaration that the broader 1.0 milestone is
finished. Exact tested revisions, not package version alone, establish the
pair; Cairn pins Harrow in [`spec/harrow-revision`](../spec/harrow-revision).

Files, JSON, dependencies and declared ID references carry numbers; JSON and
hooks also carry the `uid`. Commands accept the number in any rendering the
project declares, a full `uid`, or a `uid` prefix of at least eight hex digits
containing a letter. Migration from format 4 restores every number from
`_legacy-ids.toml`, moves each UUID to `uid`, rewrites references, renames
UUID-named files and removes the map. Commit one migration and merge it to
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
criteria, hierarchy, filter operators, UUIDs, prefixes and migrated aliases —
the format-4 contract the pinned revision was verified against.
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

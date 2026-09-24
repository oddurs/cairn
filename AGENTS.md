<!-- cairn:begin -->
## Roadmap and issues

This project tracks its roadmap and issues with `cairn`. Every item is a Markdown file under `cairn/items`, described by the schema in `cairn.toml`.

**Do not create ad-hoc TODO, PLAN or NOTES files.** Create a cairn item instead, so the work appears on the board and in the generated roadmap.

### The loop

1. `cairn next --view next` — what is ready to start. It excludes anything blocked by unfinished dependencies and puts work already in progress first.
2. `cairn claim <ID>` — take it before you start, so no one duplicates the work. `cairn claim --next --view next` picks and claims the top-ranked unclaimed item in one step, and prints its body so you can begin immediately.
3. Do the work. Record what you learn: `cairn set <ID> <field>=<value>` for fields, `cairn note <ID> "<TEXT>"` for anything that needs a sentence — why you chose something, what you tried, what to watch for.
4. `cairn tick <ID> <N>` as each acceptance criterion becomes true — `cairn show <ID> --criteria` lists them numbered. Tick what is true, not what would let you close.
5. `cairn close <ID>` when it is done, or `cairn release <ID>` to hand it back.
6. `cairn check` before you report finished. It must pass.

### Commands

```sh
cairn next --view next --json                 # ready work, ranked
cairn claim --next --view next                # take the next ready item
cairn search <TEXT> --json        # titles, bodies and labels
cairn list --json                 # all open items
cairn list --filter 'blocked=false,priority=p0'
cairn show <ID> --json            # one item, including its body
cairn new "<TITLE>" --type <TYPE> --milestone <MILESTONE>
cairn set <ID> status=<STATUS>    # also labels+=x, or any field below
cairn note <ID> "<TEXT>"          # append reasoning; never replaces
cairn show <ID> --criteria        # acceptance criteria, numbered
cairn tick <ID> <N>               # tick one; --all for every one
cairn close <ID>
cairn check                       # validate; run before finishing
cairn render                      # regenerate ROADMAP.md
```

Selection uses saved view `next`. Additional filters only narrow it; the view's sort and columns do not change `next` ranking. Over MCP, pass `{"view":"next"}` to `next_items` and to `claim_item` without an id. A direct claim is an explicit assignment outside this selection policy. Regenerate these instructions with `cairn agent --view next --write AGENTS.md`.

Claims coordinate writers in the same item directory, not separate branches, worktrees, or clones. Agree on assignments before splitting work.

Item identities are immutable UUIDv4 strings. Use full `id` values from JSON for durable references; commands also accept unambiguous prefixes of at least 8 hex digits. Store full identities in ID-reference fields, never prefixes. Migrated legacy numbers remain lookup aliases; new items do not receive numbers.

### Schema

- **Types**: `feature`, `bug`, `chore`, `docs`, `decision`, `milestone`
- **Statuses**: `backlog` (open), `planned` (open), `doing` (active), `blocked` (open), `done` (done), `dropped` (dropped)
- **`priority`**: one of p0, p1, p2, p3 — p0: data loss or broken core workflow; p1: next outcome; p2: useful; p3: optional
- **`effort`**: one of s, m, l, xl — Rough size, not an estimate
- **`sprint`**: one of s1, s2, s3, s4, s5, s6, s7, s8, s9, s10, s11, s12 — Historical durability sprint; retained for old items, not assigned to new work
- **`area`**: free text — Subsystem: workflow, git, format, integration, cli, docs, distribution, or direction
- **`due`**: date, YYYY-MM-DD — A committed date, YYYY-MM-DD; leave unset when the release is gated by evidence
- **Milestones**: `v0.1`, `v0.2`, `v0.3`, `v1.0`, `later`
- **Saved views** (`cairn list --view NAME`): `now`, `next`, `waiting`, `dependencies`, `triage`, `later`, `decisions`, `history`

### Rules

1. Before starting work, find or create the item and set it to an active status.
2. Use the fields above rather than inventing new ones; add new fields to `cairn.toml` first.
3. Never hand-edit the generated roadmap file — change items and run `cairn render`.
4. `cairn check` must pass before the work is considered done.

<!-- cairn:end -->

## Working on Cairn itself

The assessment and direction are in item 0134; decision 0142 records the product
boundary. Cairn is project memory versioned with the code, and Harrow is its
terminal counterpart. Prefer schema, queries, hooks, and the documented format
before adding another core capability.

The generated loop above describes generic dependency readiness. In this
repository, autonomous selection must use the approved queue:

```sh
cairn list --view next
cairn next --view next
cairn claim --next --view next
```

Explicit user assignments can be claimed directly. Resume your existing claim
before taking another item. Keep at most three engineering items planned, and
one active item per worker. `backlog` and `later` are not instructions to build;
`blocked` means a recorded external wait, while the `blocked` query field means
unfinished dependencies. See CONTRIBUTING.md for the full local workflow.

Set `CAIRN_AGENT` and an explicit working identity. Search finished and dropped
items with `cairn search --all` before proposing work. Keep reasoning and
verification evidence in the item; tick only what has been established.

Claims coordinate one item directory, not separate worktrees or clones.
Coordinate assignments before splitting work and review item changes with code.
Changing Harrow belongs in its own repository and tracked item.

Run `make check` before handing off code or project changes. Touching the lock,
write path, identifiers, or merge driver also requires `make durability`.
After changing the schema, regenerate this block with `cairn agent --view next
--write AGENTS.md`, run `cairn render`, and finish with `cairn check --render --strict`.

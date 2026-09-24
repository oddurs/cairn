# cairn

Project memory, versioned with the code.

Cairn keeps a project's intent, work, and decisions as Markdown files in its
repository. A schema you own gives those files structure. People and agents
use the same record; Git carries its history, review, and distribution.

Use it as a lightweight issue tracker and roadmap. Keep it because, months
later, the item still explains **why the code is like this**.

[Documentation](https://oddurs.github.io/cairn/docs) ·
[Harrow, the terminal interface](https://github.com/oddurs/harrow) ·
[Roadmap](ROADMAP.md) · [File format](spec/README.md)

![cairn](doc/demo.svg)

<sup>Recorded from real commands by `make demo`.</sup>

## The idea

An item records a problem, the reasoning behind a change, and the evidence that
it is finished. It can be a small task or a consequential decision. Closing
it makes it part of the project's memory.

Three pieces fit together:

| Piece | Responsibility |
| --- | --- |
| Cairn | The schema, queries, validated writes, and the agent workflow |
| Harrow | Watching the backlog, reading it, and making interactive decisions |
| Git | Version history, branches, review, and sharing |

The files remain useful without either program. There is no account, required
server, background process, or database to maintain.

Cairn is agent-first because an agent can discover the project's vocabulary,
find work, claim it, record evidence, and hand it back through a predictable
interface. It does not run the agent. Harrow gives the person beside that agent
a live view of the same work, and delegates changes to Cairn.

## Install

Homebrew:

```sh
brew tap oddurs/cairn
brew trust oddurs/cairn
brew install cairn
```

Or install a prebuilt binary:

```sh
curl -fsSL https://raw.githubusercontent.com/oddurs/cairn/main/install.sh | sh
```

The installer detects Linux or macOS, checks the release checksum, and installs
to `~/.local/bin`. Windows binaries are on the
[releases page](https://github.com/oddurs/cairn/releases). Release archives also
carry build provenance; the [manual](doc/cairn.texi) explains verification.

From source:

```sh
cargo install --git https://github.com/oddurs/cairn cairn-md
```

The package name is `cairn-md`; the executable is `cairn`. See
[0001](cairn/items/0001-publish-to-crates-io-and-homebrew.md) for the remaining
crates.io publication work.

Harrow is optional. Install it from
[its repository](https://github.com/oddurs/harrow#install), then run `harrow`
inside any Cairn project.

## Start small

In a new project:

```sh
cairn init --preset minimal --bare
item=$(cairn new "Document how to build this project" -q)
cairn claim "$item"
cairn edit "$item"
cairn note "$item" "Verified the instructions from a clean checkout."
cairn close "$item"
cairn render
cairn check
```

Add your problem, approach, and acceptance criteria when editing the item.
Tick criteria as they become true with `cairn tick "$item" 1`; check them with
`cairn show "$item" --criteria`.

The minimal preset is a starting point. `cairn init --bare` uses the richer
standard schema; `cairn init --from ../another-project/cairn.toml --bare`
adopts a schema you already like.

For a Git repository, set up merge support once per working copy:

```sh
cairn init --git
```

Commit the items, `cairn.toml`, and generated roadmap with the changes they
describe. Cairn does not commit for you.

## Work the record

```sh
cairn next                         # ranked, unfinished, dependency-ready work
cairn claim --next                 # atomically choose and claim in this working copy
cairn show <ID>
cairn set <ID> priority=p1
cairn note <ID> "Kept the old representation to preserve existing references."
cairn release <ID> --reason "Needs a reproduction on Windows."
cairn search --all "representation" # include finished and dropped reasoning
cairn log <ID>                     # the item's history in Git
```

`next` means dependency-ready, not necessarily approved by your project.
Select autonomous work with an explicit saved view:

```sh
cairn next --view next
cairn claim --next --view next
cairn agent --view next --write AGENTS.md
```

Define `next` in `cairn.toml` using your own statuses. It is a convention, not
a reserved view name. The view selects candidates; `next` still ranks active
work first, then priority. An additional `--filter` only narrows the selection.
MCP `next_items` and automatic `claim_item` accept the same `view` argument.
Without a view, behavior is unchanged. Views share the selection with Harrow;
they are not permissions and do not restrict explicit item assignments.

A claim coordinates writers using **the same item directory**. Separate
branches, worktrees, and clones have separate state. Agree on assignments
before splitting work; a claim is not a distributed lock.

The [Git collaboration guide](doc/COLLABORATION.md) covers assignments,
worktrees, hook locations, reference repair, rebase, cherry-pick, and review.
The [companion contract](doc/COMPATIBILITY.md) identifies the verified Harrow
revision and cross-tool checks; the old 0.1.0 binary is not the tested pair.

In this repository, start with `cairn list --view next` or
`harrow --view next`. [CONTRIBUTING.md](CONTRIBUTING.md) explains what our
statuses mean, how we select work, and how to validate it.

## A schema you own

`cairn.toml` declares types, statuses, fields, relationships, and views.
These are project choices, not a prescribed methodology. For example, this
complete small schema adds decisions and an approved queue:

```toml
format = 4

[project]
name = "my-project"
dir = "cairn/items"
default_type = "task"
default_status = "backlog"

[[type]]
name = "task"

[[type]]
name = "decision"

[[status]]
name = "backlog"
category = "open"

[[status]]
name = "planned"
category = "open"

[[status]]
name = "doing"
category = "active"

[[status]]
name = "done"
category = "done"

[[view]]
name = "next"
filter = "status=planned,blocked=false"
sort = "id"

[render]
group_by = "status"
```

Status names are yours; their categories (`open`, `active`, `done`,
`dropped`) tell the program what they mean. Fields can hold text, enums,
lists, dates, numbers, booleans, or references. Milestones are ordinary items
of a grouping type, with their own reasoning and history.

`cairn config` shows the current schema. `cairn --help` lists commands.
The [manual](doc/cairn.texi) covers immutable identities, relationships,
proposals, importing, and the full filter grammar.

## Agents

Generate instructions from the project's actual schema:

```sh
cairn agent --write AGENTS.md
```

CLI output is available as JSON, IDs, or plain tab-separated values. Use
`--limit`, filters, and targeted `show` calls to keep context small.
Search finished decisions before proposing something the project may have
already considered.

For clients using MCP, `cairn mcp --config` prints connection details and
`cairn mcp` serves the project over stdio. CLI and MCP are interfaces to the
same files; choose the one your agent uses well.

Set `CAIRN_AGENT` to identify an agent, and `CAIRN_USER` when it needs an
explicit working identity. The schema can let agents write, propose changes,
or read particular fields and statuses. These are workflow rules for
cooperating tools, not a security boundary against a process that can edit
the files.

## Built to keep

Items are plain Markdown with YAML frontmatter. The
[standalone specification](spec/README.md), frozen historical fixtures, and
[independent reader](spec/reader.py) keep the data contract separate from this
implementation.

Writes use a temporary file, flush, and rename. Mutating commands take a
local lock; read commands can still show healthy items when another file is
damaged. Tests exercise interruptions, concurrent writers, old formats, and
arbitrary inputs.

Format 4 gives every item an immutable UUIDv4, without a server or shared
counter. Type an unambiguous prefix (at least eight hex digits); store full
IDs in scripts and references. Independently created work merges without
renumbering. Conflicting edits to the same item still need review.

This development line is **1.0.0-alpha.1**, not the stable 0.3 release.
Upgrade Harrow before migrating a live project. `cairn migrate --dry-run`
previews the change; migration retains old numeric lookup aliases, filenames,
bodies, and Git history. Commit the migration once and carry it to other
branches; do not migrate them independently. See the
[migration contract](spec/README.md) and [collaboration guide](doc/COLLABORATION.md).

Extend the workflow through schema, JSON, import/export, and hooks that run
your programs. Use the array form of hooks for portable invocation. A hook
failure warns after the item is saved; it does not undo the change.

Keep validation in CI:

```yaml
- run: cairn check --render --strict
```

## Direction

The next outcome is a daily workflow that people and agents can trust across
Cairn, Harrow, and Git branches. After that, 1.0 should mean a tested
compatibility and recovery promise.

The [roadmap](ROADMAP.md) contains the selected work. The
[assessment](cairn/items/0134-give-cairn-a-durable-direction-and-a-working-project-setup.md)
records the evidence and tradeoffs. New scope should earn its maintenance cost:
prefer a schema choice, a query, a hook, or an external reader when those solve
the problem.

## The promise

<!-- promise:begin -->
cairn is free software under the MIT licence, and will remain so.

Everything a single repository can do is part of cairn, and is free: items, the
schema, the board, the roadmap, agents, merging, import and export. No
capability that belongs in cairn will be held back for something else. If
anything ever built beside cairn disappeared tomorrow, no cairn user would be
affected.

cairn does everything a single repository can do, and nothing beyond it. That
is not a policy but a description. A repository cannot see other repositories.
It cannot serve somebody who has not cloned it. It has no notion of who is
permitted to close an item, and it cannot tell you something changed while you
were not looking. Those are not capabilities withheld from cairn; they are
capabilities a directory of files does not have.

So cairn will never grow accounts, authentication, or remotes. The day that
cairn login exists, this promise has been broken.
<!-- promise:end -->

The reasoning is in [PROMISE.md](PROMISE.md).

## Development

```sh
make check          # build, tests, formatting, lints, and the project's own backlog
make durability     # also contention, long fuzzing, and independent conformance
make doc            # Info manual; requires Texinfo
```

See [CONTRIBUTING.md](CONTRIBUTING.md),
[NEWS](NEWS), [release instructions](doc/RELEASING.md), and
[SECURITY.md](SECURITY.md).

Related approaches include [git-bug](https://github.com/git-bug/git-bug),
[Backlog.md](https://github.com/MrLesk/Backlog.md),
[todo.txt](https://github.com/todotxt/todo.txt-cli), and
[dstask](https://github.com/naggie/dstask).

MIT. See [LICENSE](LICENSE).

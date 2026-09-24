# The daily loop across Git working copies

Cairn records intent and evidence beside code. Git owns transport, branches,
review and commits. No claim is a distributed lock.

## Assign before splitting

In one item directory, `claim --next --view next` chooses and claims under one
lock. A second writer sees that claim. A separate branch, linked worktree or
clone has its own files: both can successfully claim the same item.

Agree on assignments before splitting work. Carry the claim in a shared base
commit through the repository's normal review process, or explicitly assign
different items through your coordinator. Do not interpret a local successful
claim as a remote reservation. A direct user assignment can name an item
outside the saved selection view.

Resume your claim before selecting another. When handing off, record what the
next worker needs and make that change available to them:

```sh
cairn release <ID> --status planned --reason "Reproduction is in tests/example.rs; Windows still needs checking."
```

`planned` and `next` here are project conventions. Use your own schema/view.
The receiving worker reads the release note, then claims the item. An owner is
accountability for an outcome, not a competing assignee.

## Install the integration where Git expects it

```sh
cairn init --git
cairn config
git rev-parse --git-path hooks/post-merge
```

Run setup in every clone. Linked worktrees normally share the merge driver and
hook directory. With a relative `core.hooksPath`, each working tree has its own
relative hook path: run setup there too, or distribute a reviewed tracked hook.
Absolute hook paths are respected. Cairn will not overwrite somebody else's
hook; its refusal explains the two commands to add deliberately.

Git's [path resolution](https://git-scm.com/docs/git-worktree#_details) handles
the shared/private directory distinction. A tracked `.gitattributes` declares
intent, but cannot install an executable merge driver in another person's clone.

## Merge and review

Format 4 allocates random UUIDv4 identities. Two branches can create items
independently: their IDs and references stay unchanged when Git merges them.
There is no central allocator, timestamp dependency, or per-branch counter.

The installed post-merge hook regenerates the roadmap after the item merge.
It still invokes `renumber` for compatibility, but that command changes no
UUID and retains migrated filenames. Changes remain unstaged and uncommitted.

Sequence additions merge by ancestor-aware union; removals stay removed.
Conflicting scalar values and body edits stay ordinary Git conflicts. Resolve
the intended meaning, then validate:

```sh
cairn render
cairn check --render --strict
git diff
```

A manually duplicated UUID is an error. Determine whether the files are two
copies of one item or genuinely different work; do not blindly give one a new
ID and leave its incoming references pointing at the other copy.

## Rebase, cherry-pick, and forge merges

These operations do not run the successful local merge's post-merge hook.
Independent UUID creation needs no repair, but the generated roadmap may be
stale and conflicting edits still need review. Run `render` and
`check --render --strict` after the operation. Commit resulting changes
explicitly. CI should run the same validation.

Git documents the [post-merge hook's scope](https://git-scm.com/docs/githooks#_post_merge).
A clean Git status is not validation of the backlog.

## Migrate an existing project once

Upgrade Cairn and its readers, back up the complete project, and stop concurrent
writers. Preview with `cairn migrate --dry-run`, then run `cairn migrate`.
Review and commit the config, item frontmatter, and `_legacy-ids.toml`
together. Migration keeps filenames and body bytes; Git history is not rewritten.

Carry this single migration commit to other branches. Independently migrating
the same old numeric backlog produces different UUID maps and is not supported.
First reconcile legacy branches with the old writer, or replay their work
deliberately against the migrated baseline.

Old numbers remain lookup aliases, including their old prefix rendering.
New items get no numeric aliases; deleting an item never recycles its alias.
Keep the map tracked: it also lets history comparisons bridge the migration.

If interrupted, rerun `cairn migrate`. The saved
`.identity-migration.json` plan is checked against disk before resuming;
intervening edits are refused. Do not delete that plan to force normal writes.
To roll back, restore the entire pre-migration backup, not only `cairn.toml`.

## Review the decision and the code together

```sh
cairn log --range main..HEAD
cairn log <ID> --patch
git diff main...HEAD
cairn show <ID>
```

Use the actual base branch. The item says why, the code shows how, and the
acceptance criteria say what was verified. History remains ordinary Git history.
Executable examples live in `tests/collaboration.rs`: local exclusion,
committed handoff, independent worktree claims, clone setup, hook paths, branch
creation without renumbering, rebase, cherry-pick, and review.

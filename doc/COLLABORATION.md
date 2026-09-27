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

## Numbers, merges and repair

Items are numbered, and each carries a `uid` tag that never changes. A new item
takes one more than the highest number this clone can see: the working tree,
every linked worktree's uncommitted items, and every item file ever added on
any local or remote-tracking branch. Agents working in parallel, one worktree
each, therefore never collide, and a number that was deleted or is waiting on
an unmerged branch is never handed out again.

What allocation cannot see is a branch on another machine that was never
fetched. Two such branches can give the same number to different work. A
successful local merge runs the installed post-merge hook, which repairs it:

- the item already published keeps the number, and the arriving one moves to
  the next free number;
- a reference to the contested number that only the arriving side added moves
  with it;
- one that both sides added meant both items, so it keeps the number and gains
  the new one.

The tag is what makes this exact rather than a guess: it finds each item on
both sides of the merge, however its file was renamed. Changes are left
uncommitted and unstaged. Review them; Cairn never amends a merge commit.

```sh
cairn check --render --strict
git diff
```

Sequence additions merge by ancestor-aware union; removals stay removed.
Conflicting scalar values and body edits stay ordinary Git conflicts. Resolve
the intended meaning, then validate. Prose links and commit messages that
named a moved number are yours to review; the tag still resolves in any
command, so `cairn show <uid prefix>` finds the item wherever it went.

A file carrying another file's tag is a copy, and `cairn check` reports it.
Remove the copy, or delete its `uid:` line if it really is separate work.

## Rebase, cherry-pick, and forge merges

These do not run the post-merge hook. After the operation, run
`cairn renumber --dry-run`, inspect, `cairn renumber`, `render`, and
`check --render --strict`, and commit the result. Outside a merge, `renumber`
cannot tell which side a reference came from, so it leaves references on the
retained item and says so: audit them. CI should run the same validation, since
a forge merge cannot run your local hook.

Git documents the [post-merge hook's scope](https://git-scm.com/docs/githooks#_post_merge).
A clean Git status is not validation of the backlog.

## Migrate an existing project once

Back up the complete project and stop concurrent writers. Preview with
`cairn migrate --dry-run`, then run `cairn migrate`, and commit the result as
one commit.

- From format 3, every item gains one `uid:` line. Nothing else changes.
- From format 4, every item gets back the number it had, from
  `_legacy-ids.toml`; items created since get the next free numbers in creation
  order. Each UUID becomes the item's `uid`, references are rewritten to
  numbers, UUID-named files are renamed, the project's `id_format` returns, and
  the map is removed.

Carry this single migration commit to other branches rather than migrating
each one. Work filed on a branch before the migration arrives afterwards with a
UUID where its number goes; `cairn check` names it and `cairn renumber` numbers
it, keeping the UUID as its tag and rewriting every reference to it.

If interrupted, rerun `cairn migrate`. The saved `.identity-migration.json`
plan is checked against disk before resuming; intervening edits are refused.
Do not delete that plan to force normal writes.

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

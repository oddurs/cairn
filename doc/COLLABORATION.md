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
cairn release 42 --status planned --reason "Reproduction is in tests/example.rs; Windows still needs checking."
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

## Merge, repair, review

A successful local merge runs the installed post-merge hook. It repairs
duplicate numeric identifiers and regenerates the roadmap. Changes are left
uncommitted and unstaged. Review them; Cairn never amends a merge commit for you.

Renumbering cannot infer what a duplicated reference meant. If both branches
created item 42, a later `depends_on: [42]` could refer to either one. The
retained item keeps 42; **references are not automatically retargeted**.
Compare each branch's item additions against the merge base and the reported
renumber mapping. Restore the intended references with `cairn set`:

```sh
cairn renumber --dry-run
cairn renumber
cairn set 57 depends_on=56
cairn render
cairn check --render --strict
git diff
```

The IDs above are illustrative: use the actual old/new mapping. Review every
id-valued reference field, not only `depends_on`, plus prose links and commit
messages. Key-addressed references are a separate namespace; do not rewrite
them merely because a numeric ID moved. Validation detects structural problems,
not a valid reference to the wrong idea. Commit the reviewed repair explicitly.

Scalar conflicts and ambiguous edits stay ordinary Git conflicts. After
resolving one, run the same repair/validation sequence; `post-merge` is not
called for a merge that stopped with conflicts.

## Rebase, cherry-pick, and forge merges

These do not run Cairn's post-merge repair. Git may apply item files cleanly yet
leave duplicated IDs or a stale generated roadmap. After the operation finishes,
run `renumber --dry-run`, inspect, `renumber`, audit references, `render`, and
`check --render --strict`. Commit the resulting repair. Do not install automatic
commit rewriting or assume that a clean Git status proves valid Cairn data.

Git documents the [limited post-merge hook](https://git-scm.com/docs/githooks#_post_merge).
Repository CI should run `cairn check --render --strict`, since a forge merge
cannot run your local hook.

## Review the decision and the code together

```sh
cairn log --range main..HEAD
cairn log 42 --patch
git diff main...HEAD
cairn show 42
```

Use the actual base branch. The item says why, the code shows how, and the
acceptance criteria say what was verified. History remains ordinary Git history.
Executable examples live in `tests/collaboration.rs`: local exclusion,
committed handoff, independent worktree claims, clone setup, hook paths, branch
collisions with reference repair, rebase, cherry-pick, and review.

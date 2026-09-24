// Real Git operations, not approximations of what a branch leaves on disk.
mod support;
use support::*;

fn unintegrated() -> Project {
    let p = Project::new();
    git(&p, &["init", "-q", "-b", "main"]);
    p.add("Shared assignment", &[]);
    commit(&p, "base");
    p
}

fn commit(p: &Project, message: &str) {
    git(p, &["add", "-A"]);
    git(p, &["commit", "-qm", message]);
}

fn worktree(p: &Project, branch: &str) -> Project {
    let linked = Project::empty();
    git(
        p,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            branch,
            linked.root().to_str().unwrap(),
        ],
    );
    linked
}

#[test]
fn linked_worktrees_install_the_hook_git_actually_uses() {
    let p = unintegrated();
    let linked = worktree(&p, "parallel");
    linked.expect(&["init", "--git"]);
    assert!(
        p.path(".git/hooks/post-merge").exists(),
        "hooks live in the common Git directory"
    );
    assert_contains(
        &linked.expect(&["config"]).stdout,
        "integrated",
        "linked worktree detects its setup",
    );
    assert_contains(
        &linked.expect(&["init", "--git"]).all(),
        "already in place",
        "idempotent",
    );

    // A relative hooksPath is relative to each working tree, not its private
    // admin directory or the primary checkout. Spaces must remain one path.
    git(&p, &["config", "core.hooksPath", ".hooks with spaces"]);
    linked.expect(&["init", "--git"]);
    assert!(linked.path(".hooks with spaces/post-merge").exists());
    assert!(!p.path(".hooks with spaces/post-merge").exists());
    assert_contains(
        &linked.expect(&["config"]).stdout,
        "integrated",
        "respects hooksPath",
    );
}

#[test]
fn an_absolute_hooks_path_is_respected_without_overwriting_a_hook() {
    let p = unintegrated();
    let hooks = tempfile::tempdir().unwrap();
    git(
        &p,
        &["config", "core.hooksPath", hooks.path().to_str().unwrap()],
    );
    let path = hooks.path().join("post-merge");
    std::fs::write(&path, "#!/bin/sh\necho existing\n").unwrap();
    assert_contains(
        &p.fails(&["init", "--git"]).all(),
        "not cairn's",
        "no overwrite",
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "#!/bin/sh\necho existing\n"
    );
    // Another empty location demonstrates installation without deleting the
    // existing hook to make the test pass.
    let empty = tempfile::tempdir().unwrap();
    git(
        &p,
        &["config", "core.hooksPath", empty.path().to_str().unwrap()],
    );
    p.expect(&["init", "--git"]);
    assert!(empty.path().join("post-merge").exists());
    assert_contains(
        &p.expect(&["config"]).stdout,
        "integrated",
        "same resolver for diagnosis",
    );
}

#[test]
fn a_hook_that_is_not_readable_as_text_is_preserved() {
    let p = unintegrated();
    let hooks = tempfile::tempdir().unwrap();
    git(
        &p,
        &["config", "core.hooksPath", hooks.path().to_str().unwrap()],
    );
    let path = hooks.path().join("post-merge");
    std::fs::write(&path, [0xff, 0xfe]).unwrap();
    assert_contains(
        &p.fails(&["init", "--git"]).all(),
        "has not been replaced",
        "read failure is not permission to overwrite",
    );
    assert_eq!(std::fs::read(path).unwrap(), [0xff, 0xfe]);
}

#[cfg(unix)]
#[test]
fn a_disabled_hook_is_not_reported_as_integrated() {
    use std::os::unix::fs::PermissionsExt;
    let p = repository();
    std::fs::set_permissions(
        p.path(".git/hooks/post-merge"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert_contains(
        &p.fails(&["init", "--git"]).all(),
        "not executable",
        "a disabled hook is not enabled silently",
    );
    assert_contains(
        &p.expect(&["config"]).stdout,
        "not integrated",
        "Git would ignore it",
    );
}

#[test]
fn a_clone_must_opt_into_its_own_executable_integration() {
    let p = repository();
    let cloned = Project::empty();
    git(
        &p,
        &[
            "clone",
            "-q",
            p.root().to_str().unwrap(),
            cloned.root().to_str().unwrap(),
        ],
    );
    assert_contains(
        &cloned.expect(&["check"]).all(),
        "does not define it",
        "tracked intent is not executable setup",
    );
    cloned.expect(&["init", "--git"]);
    assert_contains(
        &cloned.expect(&["config"]).stdout,
        "integrated",
        "clone is configured locally",
    );
}

#[test]
fn claims_exclude_local_writers_but_do_not_reserve_other_worktrees() {
    let p = unintegrated();
    let linked = worktree(&p, "independent");
    p.expect(&["claim", &p.id(1), "--as", "first"]);
    assert_contains(
        &p.fails(&["claim", &p.id(1), "--as", "second"]).all(),
        "already claimed",
        "same-directory exclusion",
    );
    linked.expect(&["claim", &p.id(1), "--as", "second"]);
    assert_eq!(p.json(&["show", &p.id(1), "--json"])["assignee"], "first");
    assert_eq!(
        linked.json(&["show", &p.id(1), "--json"])["assignee"],
        "second"
    );
}

#[test]
fn a_committed_assignment_and_explicit_release_support_handoff_and_review() {
    let p = unintegrated();
    p.expect(&["claim", &p.id(1), "--as", "first"]);
    commit(&p, "agree on the assignment before splitting");
    let linked = worktree(&p, "implementation");
    linked.fails(&["claim", &p.id(1), "--as", "second"]);
    linked.expect(&[
        "release",
        &p.id(1),
        "--reason",
        "The reproduction is in feature.txt",
    ]);
    linked.expect(&["claim", &p.id(1), "--as", "second"]);
    linked.write(
        "feature.txt",
        "reproduction and code travel with the item\n",
    );
    linked.expect(&[
        "note",
        &p.id(1),
        "Kept the reproduction with the implementation.",
    ]);
    commit(&linked, "implement with the reasoning");
    let diff = git(&linked, &["diff", "--name-only", "main..HEAD"]);
    assert_contains(&diff, "feature.txt", "code is reviewable");
    assert_contains(&diff, "cairn/items/", "the record travels with it");
    assert_contains(
        &linked.expect(&["log", "--range", "main..HEAD"]).all(),
        "changed",
        "branch summary",
    );
    assert_contains(
        &linked.expect(&["log", &p.id(1)]).all(),
        "assignee first -> second",
        "item history",
    );
    linked.expect(&["check", "--render", "--strict"]);
}

fn id_with_title(p: &Project, title: &str) -> String {
    p.json(&["list", "--all", "--json"])
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["title"] == title)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn independent_branch_creation_preserves_identity_and_references_without_repair() {
    let p = repository();
    git(&p, &["checkout", "-qb", "branch-a"]);
    let parent = p.add("Parent A", &[]);
    p.add("Dependent A", &["-d", &parent]);
    let a = (
        id_with_title(&p, "Parent A"),
        id_with_title(&p, "Dependent A"),
    );
    commit(&p, "a with references");
    git(&p, &["checkout", "-q", "main"]);
    let parent = p.add("Parent B", &[]);
    p.add("Dependent B", &["-d", &parent]);
    let b = (
        id_with_title(&p, "Parent B"),
        id_with_title(&p, "Dependent B"),
    );
    assert_ne!(a.0, b.0);
    assert_ne!(a.1, b.1);
    commit(&p, "b with references");
    git(&p, &["merge", "--no-edit", "branch-a"]);
    for (parent, child) in [a, b] {
        assert_eq!(
            p.json(&["show", &child, "--json"])["depends_on"],
            serde_json::json!([parent])
        );
    }
    assert_eq!(p.count_all(), 5);
    p.expect(&["check", "--render", "--strict"]);
    assert!(
        git(&p, &["diff", "--name-only", "HEAD", "--", "cairn/items"])
            .trim()
            .is_empty(),
        "merging does not rewrite item identities or references"
    );
}

#[test]
fn rebase_and_cherry_pick_keep_independent_identities_without_repair() {
    for operation in ["rebase", "cherry-pick"] {
        let p = repository();
        git(&p, &["checkout", "-qb", "topic"]);
        p.add("From topic", &[]);
        let topic_id = id_with_title(&p, "From topic");
        commit(&p, "topic work");
        git(&p, &["checkout", "-q", "main"]);
        p.add("From main", &[]);
        let main_id = id_with_title(&p, "From main");
        commit(&p, "main work");
        if operation == "rebase" {
            git(&p, &["checkout", "-q", "topic"]);
            git(&p, &["rebase", "main"]);
        } else {
            git(&p, &["cherry-pick", "topic"]);
        }
        p.expect(&["check"]);
        assert_eq!(id_with_title(&p, "From topic"), topic_id);
        assert_eq!(id_with_title(&p, "From main"), main_id);
        p.expect(&["render"]);
        p.expect(&["check", "--render", "--strict"]);
        assert_eq!(p.count_all(), 3, "{operation}: nothing was lost");
    }
}

//! Migrating as one safe step, across many projects at once, and when a write
//! first needs it.
mod support;
use support::*;

/// A committed format-3 project with enough items that a dry run would rather
/// count them than name them, and a roadmap that the migration makes stale.
fn committed_format_three(items: usize) -> Project {
    let p = Project::empty();
    git(&p, &["init", "-q", "-b", "main", "."]);
    // `migrate --commit` runs git itself, outside the helper that supplies an
    // author. A machine with no global identity — CI — would refuse it.
    git(&p, &["config", "user.name", "test"]);
    git(&p, &["config", "user.email", "test@example.invalid"]);
    p.expect(&["init", "--bare", "--name", "Old"]);
    for n in 1..=items {
        p.add(&format!("Item {n}"), &[]);
    }
    p.expect(&["render", "-q"]);
    // Back to format 3 by hand: no tags, and the configuration says so.
    for f in p.files("cairn/items") {
        let path = format!("cairn/items/{f}");
        let text = p.read(&path);
        let untagged: String = text
            .split_inclusive('\n')
            .filter(|l| !l.starts_with("uid: "))
            .collect();
        p.write(&path, &untagged);
    }
    p.write(
        "cairn.toml",
        &p.read("cairn.toml").replace("format = 5", "format = 3"),
    );
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "format 3"]);
    p
}

fn head(p: &Project) -> String {
    git(p, &["rev-parse", "HEAD"]).trim().to_string()
}

#[test]
fn uncommitted_changes_stop_a_migration_before_anything_is_written() {
    let p = committed_format_three(2);
    p.append("cairn/items/0001-item-1.md", "An edit nobody committed.\n");
    let before = p.read("cairn/items/0002-item-2.md");

    let out = p.fails(&["migrate"]);
    assert_contains(&out.all(), "uncommitted changes", "it says why");
    assert_contains(&out.all(), "0001-item-1.md", "and which file");
    assert_eq!(
        p.read("cairn/items/0002-item-2.md"),
        before,
        "nothing moved"
    );
    assert_contains(&p.read("cairn.toml"), "format = 3", "");

    p.expect(&["migrate", "--allow-dirty"]);
    assert_contains(&p.read("cairn.toml"), "format = 5", "when asked to");
}

#[test]
fn a_dry_run_counts_what_it_would_touch_and_names_branches_with_items() {
    let p = committed_format_three(12);
    git(&p, &["checkout", "-qb", "elsewhere"]);
    p.write_item(
        "0013-from-a-branch.md",
        "id: 13\ntitle: From a branch\nstatus: backlog",
        "",
    );
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "branch work"]);
    git(&p, &["checkout", "-q", "main"]);

    let out = p.expect(&["migrate", "--dry-run"]).stdout;
    assert_contains(&out, "12 existing item(s) would be rewritten", "counted");
    assert_contains(&out, "`--verbose` names them", "and how to see them");
    assert_missing(&out, "0007-item-7.md", "a long list is not printed");
    assert_contains(&out, "elsewhere (1)", "the branch that carries an item");

    let out = p.expect(&["migrate", "--dry-run", "--verbose"]).stdout;
    assert_contains(&out, "0007-item-7.md", "named when asked");
}

#[test]
fn a_migration_reports_what_it_verified_rerenders_and_commits_once() {
    let p = committed_format_three(3);
    let before = head(&p);

    let out = p.expect(&["migrate", "--commit"]).stdout;
    assert_contains(&out, "verified:", "what was proved before writing");
    assert_contains(&out, "3 item(s) keep their bodies", "");
    assert_contains(&out, "committed:", "");

    assert_eq!(
        git(&p, &["rev-list", "--count", &format!("{before}..HEAD")]).trim(),
        "1",
        "one commit"
    );
    assert!(
        git(&p, &["status", "--porcelain"]).trim().is_empty(),
        "nothing left behind"
    );
    let message = git(&p, &["log", "-1", "--format=%B"]);
    assert_contains(&message, "migrate from format 3 to 5", "");
    assert_contains(&message, "Verified before writing", "");
    p.expect(&["check", "--render", "--strict"]);
}

#[test]
fn without_commit_it_says_what_to_do_next() {
    let p = committed_format_three(1);
    let out = p.expect(&["migrate"]).stdout;
    assert_contains(&out, "next:", "");
    assert_contains(
        &out,
        "git add -A cairn.toml cairn/items ROADMAP.md",
        "the exact command",
    );
}

#[test]
fn migrate_all_reports_every_project_and_migrates_the_ones_it_can() {
    let root = Project::empty();
    let make = |name: &str, p: &Project| {
        let to = root.path(name);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::rename(p.root(), &to).unwrap();
    };
    // Kept alive until the end: each owns a temporary directory that has
    // been moved, so dropping one early removes nothing that matters.
    let old = committed_format_three(2);
    make("apps/old", &old);
    let current = Project::new();
    current.add("Already current", &[]);
    make("current", &current);
    let dirty = committed_format_three(1);
    dirty.append("cairn/items/0001-item-1.md", "Uncommitted.\n");
    make("dirty", &dirty);
    let future = Project::new();
    future.write(
        "cairn.toml",
        &future
            .read("cairn.toml")
            .replace("format = 5", "format = 99"),
    );
    make("future", &future);
    // Nothing under a hidden or build directory is a project of its own.
    let hidden = Project::new();
    make(".worktrees/copy", &hidden);

    let out = root.expect(&["migrate", "--all", "--dry-run"]).stdout;
    assert_contains(&out, "apps/old", "found at depth");
    assert_contains(&out, "format 3 -> 5", "what it would do");
    assert_contains(&out, "current (format 5)", "");
    assert_contains(&out, "skip: 1 uncommitted change(s)", "");
    assert_contains(&out, "format 99 needs a newer cairn", "");
    assert_missing(&out, ".worktrees", "hidden directories are not searched");
    assert_contains(&out, "1 to migrate, 1 current, 2 skipped", "");
    assert_contains(
        &std::fs::read_to_string(root.path("apps/old/cairn.toml")).unwrap(),
        "format = 3",
        "a dry run",
    );

    let out = root.expect(&["migrate", "--all", "--commit"]).stdout;
    assert_contains(&out, "1 migrated, 1 current, 2 skipped, 0 failed", "");
    assert_contains(
        &std::fs::read_to_string(root.path("apps/old/cairn.toml")).unwrap(),
        "format = 5",
        "",
    );
    assert_contains(
        &std::fs::read_to_string(root.path("dirty/cairn.toml")).unwrap(),
        "format = 3",
        "skipped, untouched",
    );
    drop((old, current, dirty, future, hidden));
}

#[test]
fn migrate_all_carries_on_past_a_failure_and_says_so() {
    let root = Project::empty();
    let broken = Project::new();
    broken.write(
        "cairn.toml",
        &broken
            .read("cairn.toml")
            .replace("format = 5", "format = 4"),
    );
    // A format-4 item naming something that is not there: the 4-to-5 step
    // refuses a backlog that does not check.
    broken.write_item(
        "a.md",
        "id: c93f26b1-0000-4000-8000-000000000009\ntitle: Dangling\nstatus: backlog\n\
         depends_on: [d93f26b1-0000-4000-8000-00000000000d]",
        "",
    );
    let to = root.path("broken");
    std::fs::rename(broken.root(), &to).unwrap();
    let out = root.fails(&["migrate", "--all"]);
    assert_contains(&out.all(), "0 migrated", "");
    assert_contains(&out.all(), "1 failed", "");
    drop(broken);
}

#[test]
fn a_write_that_nobody_can_answer_still_refuses() {
    let p = committed_format_three(1);
    // Not a terminal: the test harness pipes both streams.
    let out = p.fails(&["new", "Not yet"]);
    assert_contains(&out.all(), "cairn migrate", "the way forward");
    assert_missing(&out.all(), "migrate now?", "nobody is there to answer");
}

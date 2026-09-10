// cairn — end-to-end tests: repository.
//
// Copyright (c) 2026 Oddur Sigurdsson. MIT licensed; see LICENSE.
//
// cairn on top of git: history, merging, hooks, identifier repair, and what
// happens when two people write at once.
//
// These drive the real binary, so they cover argument parsing, exit codes and
// stderr the way a user meets them, and they avoid a shell so they run on every
// supported platform. The harness they share is in `tests/support/`.
mod support;
#[allow(unused_imports)]
use support::*;

#[allow(unused_imports)]
use std::path::{Path, PathBuf};
#[allow(unused_imports)]
use std::process::{Command, Stdio};

// --- renumber ---------------------------------------------------------------

#[test]
fn renumber_is_a_no_op_when_ids_are_unique() {
    let p = seeded();
    assert_contains(
        &p.expect(&["renumber"]).all(),
        "no duplicate ids",
        "says so",
    );
}

#[test]
fn renumber_repairs_a_merge_collision() {
    let p = Project::new();
    p.add("Already here", &[]);
    // What a merge of two branches produces: same id, different filename.
    p.write(
        "cairn/items/0001-arrived-from-another-branch.md",
        "---\nid: 1\ntitle: Arrived from another branch\nstatus: backlog\n---\nbody\n",
    );
    p.fails(&["check"]);

    let dry = p.expect(&["renumber", "--dry-run"]);
    assert_contains(&dry.all(), "->", "the plan is shown");
    p.fails(&["check"]);

    p.expect(&["renumber"]);
    p.expect(&["check"]);
    assert_eq!(
        p.json(&["show", "1", "--json"])["title"],
        "Already here",
        "the older item keeps the contested id"
    );
}

/// Gaps in the identifier sequence are permanent, and that is the design.
///
/// `renumber --compact` used to close them. It was removed in the surface
/// review before 1.0: it moved every id in the project, which silently breaks
/// every commit message, pull request and human memory that referred to one,
/// and cairn has no way to rewrite any of those. That is a large, irreversible
/// cost for a cosmetic benefit, and nothing outside its own test ever wanted it.
#[test]
fn renumber_repairs_duplicates_and_leaves_gaps_alone() {
    let p = Project::new();
    for n in 1..=4 {
        p.add(&format!("Item {n}"), &[]);
    }
    p.expect(&["remove", "2", "--force"]);
    p.write(
        "cairn/items/0001-collision.md",
        "---\nid: 1\ntitle: Collision\nstatus: backlog\n---\nbody\n",
    );

    p.expect(&["renumber"]);
    p.expect(&["check"]);

    // The gap left by removing item 2 is still there, and the duplicate has
    // been given an id of its own.
    let ids: Vec<u32> = p
        .expect(&["list", "-A", "--ids"])
        .lines()
        .iter()
        .filter_map(|l| l.trim().parse().ok())
        .collect();
    assert!(!ids.contains(&2), "the gap was closed: {ids:?}");
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len(), "duplicates survived: {ids:?}");

    // And the flag is gone rather than quietly accepted.
    let out = p.fails(&["renumber", "--compact"]);
    assert_contains(
        &out.all(),
        "unexpected argument",
        "--compact should no longer exist",
    );
}
// --- hooks ------------------------------------------------------------------

/// A hook that writes into a second project, using the cairn binary itself.
/// Nothing else is guaranteed to exist on every platform, and it means the hook
/// tests need no shell at all.
fn sidecar_hook(sidecar: &Path, extra: &[&str]) -> String {
    let mut argv = vec![
        bin().to_string(),
        "-C".into(),
        sidecar.display().to_string(),
        "new".into(),
    ];
    argv.extend(extra.iter().map(|s| s.to_string()));
    argv.push("-q".into());
    let quoted: Vec<String> = argv
        .iter()
        .map(|a| format!("{:?}", a.replace('\\', "/")))
        .collect();
    format!("[{}]", quoted.join(", "))
}

#[test]
fn hooks_fire_in_the_portable_argv_form() {
    let side = Project::new();
    let p = Project::new();
    p.set_hooks(&format!(
        "after-create = {}\nafter-change = {}\n",
        sidecar_hook(side.root(), &["created"]),
        sidecar_hook(side.root(), &["changed"]),
    ));

    p.add("Triggers a hook", &[]);
    assert_eq!(side.count_all(), 1, "after-create fired");
    p.expect(&["set", "1", "status=doing", "-q"]);
    assert_eq!(side.count_all(), 2, "after-change fired");
}

#[test]
fn a_hook_receives_the_item_as_json_on_stdin() {
    let side = Project::new();
    let p = Project::new();
    // `new --stdin` makes the hook's stdin the new item's body, so whatever the
    // hook was handed becomes observable without involving a shell.
    p.set_hooks(&format!(
        "after-create = {}\n",
        sidecar_hook(side.root(), &["captured", "--stdin"]),
    ));

    p.add("Distinctive title", &[]);
    let captured = side.json(&["show", "1", "--json"]);
    assert_contains(
        captured["body"].as_str().unwrap(),
        "Distinctive title",
        "the hook was handed the item as JSON",
    );
}

#[test]
fn hooks_receive_the_event_in_the_environment() {
    // The one test that must use the shell form, because reading an environment
    // variable is the thing being checked — and that syntax is per-platform.
    let p = Project::new();
    let script = if cfg!(windows) {
        "echo %CAIRN_ITEM_ID% %CAIRN_EVENT%> hook-env.txt"
    } else {
        "echo $CAIRN_ITEM_ID $CAIRN_EVENT > hook-env.txt"
    };
    p.set_hooks(&format!("after-create = {script:?}\n"));

    p.add("Env", &[]);
    let recorded = p.read("hook-env.txt");
    assert_contains(&recorded, "0001", "the item id");
    assert_contains(&recorded, "after-create", "the event name");
}

#[test]
fn hooks_can_be_suppressed() {
    let side = Project::new();
    let p = Project::new();
    p.set_hooks(&format!(
        "after-create = {}\n",
        sidecar_hook(side.root(), &["fired"])
    ));

    p.expect(&["--no-hooks", "new", "Quiet", "-q"]);
    assert_eq!(side.count_all(), 0, "--no-hooks");

    let out = Command::new(bin())
        .args(["new", "Also quiet", "-q"])
        .current_dir(p.root())
        .env("NO_COLOR", "1")
        .env("CAIRN_NO_HOOKS", "1")
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(side.count_all(), 0, "CAIRN_NO_HOOKS");
}

#[test]
fn a_failing_hook_warns_without_failing_the_command() {
    let p = Project::new();
    // A command that does not exist: the failure mode a user actually hits.
    p.set_hooks("after-create = [\"cairn-no-such-program\"]\n");

    let out = p.expect(&["new", "Still works", "-q"]);
    assert_contains(&out.all(), "warning:", "the failure is reported");
    assert_eq!(p.count(), 1, "the item was still created");
}

#[test]
fn an_empty_hook_is_reported() {
    let p = Project::new();
    p.set_hooks("after-create = []\n");
    assert_contains(
        &p.expect(&["new", "Empty hook", "-q"]).all(),
        "empty command",
        "not silently ignored",
    );
}
// --- concurrency ------------------------------------------------------------

/// Run the same command from several processes at once and collect the results.
fn race(p: &Project, invocations: Vec<Vec<String>>) -> Vec<Out> {
    let children: Vec<_> = invocations
        .into_iter()
        .map(|args| {
            Command::new(bin())
                .args(&args)
                .current_dir(p.root())
                .env("NO_COLOR", "1")
                .env("PATH", path_with_binary())
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("spawn")
        })
        .collect();
    children
        .into_iter()
        .map(|c| {
            let out = c.wait_with_output().expect("wait");
            Out {
                code: out.status.code().unwrap_or(-1),
                stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            }
        })
        .collect()
}

#[test]
fn concurrent_creates_never_collide() {
    // Allocating an id means reading the highest and adding one, which is a
    // race unless something serialises it. Without the lock this produces
    // duplicates reliably.
    let p = Project::new();
    let invocations: Vec<Vec<String>> = (0..40)
        .map(|n| vec!["new".into(), format!("Concurrent item {n}"), "-q".into()])
        .collect();
    let results = race(&p, invocations);

    let succeeded = results.iter().filter(|r| r.ok()).count();
    assert_eq!(succeeded, 40, "every writer got its turn");
    assert_eq!(p.count_all(), 40, "and every write landed");

    let ids = p.expect(&["list", "-A", "--ids"]).lines();
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), 40, "no id was handed out twice");
    p.expect(&["check"]);
}

#[test]
fn concurrent_claimers_produce_exactly_one_holder() {
    let p = Project::new();
    let id = p.add("Contested", &[]);
    let invocations: Vec<Vec<String>> = (0..12)
        .map(|n| {
            vec![
                "claim".into(),
                id.clone(),
                "--as".into(),
                format!("agent-{n}"),
                "-q".into(),
            ]
        })
        .collect();
    let results = race(&p, invocations);

    let winners = results.iter().filter(|r| r.ok()).count();
    assert_eq!(winners, 1, "one claimer won; the rest were told why");
    for r in results.iter().filter(|r| !r.ok()) {
        assert_contains(
            &r.all(),
            "already claimed by",
            "the loser is told who holds it",
        );
    }
    let holder = p.json(&["show", &id, "--json"])["assignee"].clone();
    assert!(
        holder.is_string(),
        "the item has exactly one owner: {holder}"
    );
    p.expect(&["check"]);
}

#[test]
fn concurrent_mixed_writes_leave_a_valid_backlog() {
    let p = Project::new();
    for n in 0..6 {
        p.add(&format!("Existing {n}"), &[]);
    }
    let mut invocations: Vec<Vec<String>> = Vec::new();
    for n in 0..10 {
        invocations.push(vec!["new".into(), format!("Added {n}"), "-q".into()]);
        invocations.push(vec![
            "set".into(),
            format!("{}", (n % 6) + 1),
            "status=doing".into(),
            "-q".into(),
        ]);
        invocations.push(vec!["claim".into(), "--next".into(), "-q".into()]);
    }
    race(&p, invocations);

    p.expect(&["check"]);
    assert!(p.count_all() >= 16, "nothing was lost");
    for entry in std::fs::read_dir(p.path("cairn/items")).unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().to_string();
        assert!(!name.ends_with(".tmp"), "no partial write survived: {name}");
    }
}

#[test]
fn reads_are_never_blocked_by_a_writer() {
    // A held lock must not make the backlog unlistable.
    let p = seeded();
    p.write(
        "cairn/items/.lock",
        &format!("pid 999999\nsince {}\n", now_secs()),
    );
    for args in [
        vec!["list", "--count"],
        vec!["next"],
        vec!["search", "item"],
        vec!["show", "1"],
    ] {
        assert!(
            p.run(&args).ok(),
            "cairn {args:?} waited on a lock it should ignore"
        );
    }
    p.remove("cairn/items/.lock");
}

#[test]
fn a_held_lock_stops_a_writer_with_an_explanation() {
    let p = seeded();
    p.write(
        "cairn/items/.lock",
        &format!("pid 999999\nsince {}\n", now_secs()),
    );
    let out = p.fails(&["new", "Blocked", "-q"]);
    assert_contains(&out.all(), "another cairn process", "what is happening");
    assert_contains(&out.all(), ".lock", "and where to look");
    p.remove("cairn/items/.lock");
}

#[test]
fn a_stale_lock_is_broken_rather_than_waited_on() {
    // A process that died holding the lock must not wedge the project forever.
    let p = seeded();
    p.write("cairn/items/.lock", "pid 999999\nsince 1000000000\n");
    let out = p.expect(&["new", "Proceeds anyway", "-q"]);
    assert_contains(&out.all(), "breaking a lock", "it says what it did");
    assert_eq!(p.count(), 4);
}

#[test]
fn the_lock_is_released_when_a_command_finishes() {
    let p = seeded();
    p.expect(&["set", "1", "status=doing", "-q"]);
    assert!(
        !p.exists("cairn/items/.lock"),
        "the lock did not outlive the command"
    );
    p.expect(&["new", "Another", "-q"]);
    assert!(!p.exists("cairn/items/.lock"));
}

#[test]
fn a_hook_may_call_cairn_without_deadlocking() {
    // Hooks run after the write is durable and after the lock is released,
    // precisely so a hook that shells back into cairn cannot block on its own
    // parent. This is the test that keeps that ordering honest.
    let p = Project::new();
    let mut argv = vec![
        bin().to_string(),
        "-C".into(),
        p.root().display().to_string(),
    ];
    argv.extend(["new".to_string(), "written by the hook".into(), "-q".into()]);
    let quoted: Vec<String> = argv
        .iter()
        .map(|a| format!("{:?}", a.replace('\\', "/")))
        .collect();
    // Only fires for the first item; the hook's own `new` runs with hooks
    // suppressed, so this does not recurse.
    p.set_hooks(&format!("after-create = [{}]\n", quoted.join(", ")));

    let out = p.expect(&["new", "Triggers the hook", "-q"]);
    assert!(
        !out.all().contains("another cairn process"),
        "no deadlock: {}",
        out.all()
    );
    assert_eq!(p.count_all(), 2, "both the item and the hook's item exist");
    assert!(!p.exists("cairn/items/.lock"));
}

#[test]
fn the_lock_is_not_mistaken_for_an_item() {
    let p = Project::new();
    p.add("Real", &[]);
    p.write("cairn/items/.lock", "pid 1\nsince 1000000000\n");
    assert_eq!(p.count_all(), 1);
    p.expect(&["check"]);
    assert_contains(
        &p.read("cairn/items/.gitignore"),
        ".lock",
        "and it is kept out of the repository",
    );
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
// --- git integration --------------------------------------------------------

/// Run git in the project, requiring success.
fn git(p: &Project, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(p.root())
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
        .env("PATH", path_with_binary())
        .output()
        .unwrap_or_else(|e| panic!("git {args:?}: {e}"));
    assert!(
        out.status.success(),
        "git {args:?} failed:\n{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A repository with cairn set up and one committed item.
fn repository() -> Project {
    let p = Project::empty();
    git(&p, &["init", "-q", "-b", "main", "."]);
    p.expect(&["init", "--bare", "--name", "Merged", "--git"]);
    p.add("Base", &[]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "base"]);
    p
}

#[test]
fn branches_that_both_add_an_item_merge_without_a_conflict() {
    // Before this, the first parallel merge produced a conflict in ROADMAP.md
    // and two items claiming the same id. Neither is really a conflict: both
    // files are derived, so the answer is to derive them again.
    let p = repository();

    git(&p, &["checkout", "-qb", "branch-a"]);
    p.add("From A", &[]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "a"]);

    git(&p, &["checkout", "-q", "main"]);
    git(&p, &["checkout", "-qb", "branch-b"]);
    p.add("From B", &[]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "b"]);

    // The merge itself must succeed: no conflict markers, no manual step.
    git(&p, &["merge", "--no-edit", "branch-a"]);

    p.expect(&["check"]);
    p.expect(&["render", "--check", "-q"]);

    let ids = p.expect(&["list", "-A", "--ids"]).lines();
    assert_eq!(ids.len(), 3, "every item survived the merge");
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), 3, "and the collision was repaired");

    let roadmap = p.read("ROADMAP.md");
    for title in ["Base", "From A", "From B"] {
        assert_contains(&roadmap, title, "the roadmap was re-derived, not merged");
    }
    assert!(!roadmap.contains("<<<<<<<"), "no conflict markers survived");
}

#[test]
fn the_git_setup_is_idempotent_and_adoptable() {
    let p = Project::empty();
    git(&p, &["init", "-q", "-b", "main", "."]);
    // Adopting it on a project that already exists must not re-run init.
    p.expect(&["init", "--bare", "--name", "Existing"]);
    p.add("Already here", &[]);

    let first = p.expect(&["init", "--git"]);
    assert_contains(&first.all(), "merge driver", "it reports what it changed");
    let second = p.expect(&["init", "--git"]);
    assert_contains(&second.all(), "already in place", "and does nothing twice");

    assert_eq!(p.count_all(), 1, "the existing backlog was left alone");
    assert_contains(
        &p.read(".gitattributes"),
        "merge=cairn",
        "attributes written",
    );
    assert!(p.path(".git/hooks/post-merge").exists(), "hook installed");
    assert_contains(
        &p.expect(&["config"]).stdout,
        "integrated",
        "and the state is visible",
    );
}

#[test]
fn the_setup_refuses_to_overwrite_someone_elses_hook() {
    let p = Project::empty();
    git(&p, &["init", "-q", "-b", "main", "."]);
    p.expect(&["init", "--bare", "--name", "Hooked"]);
    std::fs::create_dir_all(p.path(".git/hooks")).unwrap();
    p.write(
        ".git/hooks/post-merge",
        "#!/bin/sh\necho someone else's hook\n",
    );

    let out = p.fails(&["init", "--git"]);
    assert_contains(&out.all(), "not cairn's", "it says why");
    assert_contains(&out.all(), "cairn renumber", "and what to add by hand");
    assert_contains(
        &p.read(".git/hooks/post-merge"),
        "someone else",
        "the existing hook is untouched",
    );
}

#[test]
fn outside_a_repository_the_setup_says_so() {
    let p = Project::new();
    assert_contains(
        &p.fails(&["init", "--git"]).all(),
        "not a git repository",
        "rather than failing obscurely",
    );
}

#[test]
fn closing_upstream_only_applies_to_a_tracker() {
    // --close reaches out to GitHub through `gh`; asking for it on a JSON
    // document is a mistake worth catching before anything is written.
    let p = Project::new();
    p.write("items.json", r#"[{"title": "From a file"}]"#);
    let out = p.fails(&["import", "--from", "json", "items.json", "--close"]);
    assert_contains(&out.all(), "only applies to", "it says why");
    assert_eq!(p.count_all(), 0, "and nothing was imported");
}

#[test]
fn a_repeated_import_has_nothing_left_to_close() {
    // Provenance makes the second run a no-op, so no issue is commented twice.
    // This is why idempotence and --close compose without extra bookkeeping.
    let p = Project::new();
    p.write(
        "items.json",
        r#"[{"title": "Once", "source": "github:owner/repo#1"}]"#,
    );
    p.expect(&["import", "--from", "json", "items.json", "-q"]);
    let second = p.expect(&["import", "--from", "json", "items.json"]);
    assert_contains(&second.all(), "1 already present", "nothing to do");
    assert_eq!(p.count_all(), 1);
}

#[test]
fn mcp_records_work_under_the_name_the_client_gave() {
    // Found by driving the server as a client: a claim over MCP was recorded
    // against `git config user.name`, so an agent's work appeared in the backlog
    // under the repository owner's name. The protocol already carries the
    // answer — `clientInfo.name` in initialize.
    let p = Project::new();
    p.add("Something to take", &[]);
    let replies = p.mcp(
        &[
            r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","clientInfo":{"name":"some-agent","version":"1"}}}"#,
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"claim_item","arguments":{"id":1}}}"#,
        ],
    );
    assert_eq!(replies[1]["result"]["isError"], false);
    assert_eq!(
        p.json(&["show", "1", "--json"])["assignee"],
        "some-agent",
        "the client's own name, not the repository owner's"
    );
}

#[test]
fn an_explicit_identity_still_wins_over_the_client_name() {
    let p = Project::new();
    p.add("Something to take", &[]);
    let replies = p.mcp(
        &[
            r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"clientInfo":{"name":"some-agent"}}}"#,
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"claim_item","arguments":{"id":1,"as":"a-person"}}}"#,
        ],
    );
    assert_eq!(replies[1]["result"]["isError"], false);
    assert_eq!(p.json(&["show", "1", "--json"])["assignee"], "a-person");
}
// --- an item's history ------------------------------------------------------

/// The central claim is that the repository is the database. This is the part
/// of that claim a database cannot make, so it had better work.
#[test]
fn the_history_of_an_item_reads_as_field_changes() {
    let p = repository();
    p.add("Support OAuth", &[]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "add the item"]);

    p.expect(&["set", "2", "status=doing"]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "start it"]);

    p.expect(&["close", "2"]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "finish it"]);

    let out = p.expect(&["log", "2"]);
    let text = out.stdout.clone();
    assert_contains(&text, "created", "the first revision is the creation");
    assert_contains(&text, "status backlog -> doing", "the transition it made");
    assert_contains(&text, "-> done", "and the one that closed it");

    // A patch would be the lazy answer, and would say nothing a reader wants.
    assert!(
        !text.contains("@@") && !text.contains("+++"),
        "the default output is a diff rather than a summary:\n{text}"
    );

    // `updated` changes on every single write, so reporting it would put a line
    // of noise under every real change.
    assert!(
        !text.contains("updated 2026") && !text.contains("updated ->"),
        "the `updated` stamp is reported as a change:\n{text}"
    );
}

/// Renaming the file when a title changes is a feature. Without following
/// renames, using it would silently destroy the item's history — so this is the
/// test that says why `--follow` is there.
#[test]
fn history_survives_the_rename_a_retitle_causes() {
    let p = repository();
    p.add("Frist draft", &[]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "with a typo in the title"]);

    let before = p.expect(&["show", "2", "--path"]).trimmed();
    p.expect(&["set", "2", "title=Second draft"]);
    let after = p.expect(&["show", "2", "--path"]).trimmed();
    assert_ne!(before, after, "a retitle should have renamed the file");

    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "fix the title"]);

    let text = p.expect(&["log", "2"]).stdout;
    assert_contains(&text, "created", "the creation is still in the history");
    assert_contains(
        &text,
        "title",
        "and the retitle that renamed the file is reported",
    );
    assert_eq!(
        text.lines().filter(|l| l.contains("created")).count(),
        1,
        "the item was created once, not once per name:\n{text}"
    );
}

/// cairn does not require git, so a project without it must explain rather than
/// fail: an error here would make the tool look broken on a legitimate setup.
#[test]
fn history_outside_a_repository_explains_itself() {
    let p = Project::new();
    p.add("Not versioned", &[]);

    let out = p.expect(&["log", "1"]);
    assert!(out.ok(), "this is not a failure: {}", out.all());
    assert_contains(&out.stdout, "no history", "it says there is none");
    assert_contains(
        &out.stdout,
        "not in a git repository",
        "and says why, rather than reporting that a program could not be run",
    );
}

/// An item created but not yet committed has no history, which is different
/// from an item whose history cannot be read.
#[test]
fn an_uncommitted_item_says_so_rather_than_showing_nothing() {
    let p = repository();
    p.add("Brand new", &[]);

    let out = p.expect(&["log", "2"]);
    assert_contains(
        &out.stdout,
        "not committed",
        "a new item has no history, which is not the same as having none to read",
    );
}

/// A committed item edited since is the normal state of a working tree, and the
/// history is incomplete without saying so.
#[test]
fn history_reports_a_working_tree_that_has_moved_on() {
    let p = repository();
    p.add("Committed", &[]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "commit it"]);

    let clean = p.expect(&["log", "2"]).stdout;
    assert!(
        !clean.contains("working tree differs"),
        "nothing had changed yet:\n{clean}"
    );

    p.expect(&["set", "2", "priority=p0"]);
    let dirty = p.expect(&["log", "2"]).stdout;
    assert_contains(
        &dirty,
        "working tree differs",
        "an edited item should say the last commit is not the whole story",
    );
}

#[test]
fn history_is_available_as_json() {
    let p = repository();
    p.add("Machine readable", &[]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "add"]);
    p.expect(&["set", "2", "status=doing"]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "move"]);

    let out = p.expect(&["log", "2", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&out.stdout)
        .unwrap_or_else(|e| panic!("not JSON: {e}\n{}", out.stdout));

    assert_eq!(v["available"], true);
    let revisions = v["revisions"].as_array().expect("revisions");
    assert_eq!(revisions.len(), 2, "two commits touched it");
    assert_eq!(revisions[0]["changes"][0]["field"], "created");
    assert_eq!(revisions[1]["changes"][0]["field"], "status");
    assert_eq!(revisions[1]["changes"][0]["to"], "doing");

    // Outside a repository the shape has to stay parseable, or a caller has to
    // special-case the thing it is least likely to have tested.
    let bare = Project::new();
    bare.add("Elsewhere", &[]);
    let out = bare.expect(&["log", "1", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&out.stdout).expect("still JSON");
    assert_eq!(v["available"], false);
    assert!(v["revisions"].as_array().expect("revisions").is_empty());
}

#[test]
fn history_can_be_limited_to_the_most_recent_revisions() {
    let p = repository();
    p.add("Busy", &[]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "one"]);
    for status in ["doing", "backlog", "doing"] {
        p.expect(&["set", "2", &format!("status={status}")]);
        git(&p, &["add", "-A"]);
        git(&p, &["commit", "-qm", "change"]);
    }

    let all = p.expect(&["log", "2"]).stdout;
    let some = p.expect(&["log", "2", "-n", "2"]).stdout;
    assert!(
        some.lines().count() < all.lines().count(),
        "-n did not limit anything:\n{some}"
    );
    assert!(
        !some.contains("created"),
        "-n 2 should show the two most recent, not the two oldest:\n{some}"
    );
}

/// Every item cairn writes has the same shape, and a fresh one is mostly
/// boilerplate — similar enough that git's rename detection concludes item 2
/// was renamed from item 1 and follows into the wrong item's history.
///
/// This is not a hypothetical: `git log --follow` does exactly that on a
/// two-item project, which is every project. Attributing one item's creation to
/// another is worse than showing no history at all, because it looks right.
#[test]
fn history_does_not_wander_into_a_different_item() {
    let p = repository();
    p.add("Second item", &[]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "add the second"]);
    p.expect(&["set", "2", "status=doing"]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "start the second"]);

    let out = p.expect(&["log", "2", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&out.stdout).expect("JSON");
    let revisions = v["revisions"].as_array().expect("revisions");

    assert_eq!(
        revisions.len(),
        2,
        "item 2 has two commits; git's rename detection offers item 1's as well:\n{}",
        out.stdout
    );
    for rev in revisions {
        let path = rev["path"].as_str().unwrap_or_default();
        assert!(
            path.contains("0002"),
            "a revision of a different item leaked in: {path}"
        );
    }
}

/// In a shallow clone the oldest revision on hand is a horizon, not a
/// beginning. Calling it "created" states something false with complete
/// confidence, which is the worst way for a history to be wrong.
#[test]
fn a_shallow_clone_does_not_claim_a_creation_it_cannot_see() {
    let p = repository();
    p.add("Long lived", &[]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "create it"]);
    p.expect(&["set", "2", "status=doing"]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "and change it"]);

    let clone = Project::empty();
    let url = format!("file://{}", p.root().display());
    let out = Command::new("git")
        .args(["clone", "-q", "--depth", "1", &url, "."])
        .current_dir(clone.root())
        .output()
        .expect("git clone");
    if !out.status.success() {
        // Some sandboxes refuse file:// clones; that is not this test's
        // subject, and failing here would be noise.
        eprintln!("skipping: shallow clone unavailable in this environment");
        return;
    }

    let shown = clone.expect(&["log", "2"]);
    assert!(
        !shown.stdout.contains("created"),
        "the creating commit is not in this clone, so it must not be claimed:\n{}",
        shown.stdout
    );
    assert_contains(
        &shown.all(),
        "shallow",
        "and the reason the history stops has to be said",
    );

    let v: serde_json::Value =
        serde_json::from_str(&clone.expect(&["log", "2", "--json"]).stdout).expect("JSON");
    assert_eq!(v["truncated"], true, "a caller can tell too");
}
// --- merging items ----------------------------------------------------------

/// Run a merge that is expected to conflict, without asserting it succeeded.
fn merge(p: &Project, branch: &str) -> Out {
    let out = Command::new("git")
        .args(["merge", "--no-edit", branch])
        .current_dir(p.root())
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
        .env("PATH", path_with_binary())
        .output()
        .expect("git merge");
    Out {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

fn on_branch(p: &Project, name: &str, from: &str, work: &[&str]) {
    git(p, &["checkout", "-q", from]);
    git(p, &["checkout", "-qb", name]);
    p.expect(work);
    git(p, &["add", "-A"]);
    git(p, &["commit", "-qm", name]);
}

/// Two branches each adding to the same sequence both meant what they added,
/// and neither meant to remove the other's.
#[test]
fn two_branches_adding_to_a_sequence_merge_by_union() {
    let p = repository();
    p.add("OAuth", &[]);
    p.add("Q3 security", &[]);
    p.add("Work", &[]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "three items"]);

    on_branch(&p, "one", "main", &["set", "4", "part_of=2"]);
    on_branch(&p, "two", "main", &["set", "4", "part_of=3"]);

    git(&p, &["checkout", "-q", "main"]);
    git(&p, &["merge", "--no-edit", "one"]);
    let second = merge(&p, "two");
    assert!(
        second.ok(),
        "the union has an answer, so this should not conflict:\n{}",
        second.all()
    );

    let raw = p.expect(&["show", "4", "--raw"]).stdout;
    assert_contains(&raw, "- 2", "the first branch's edge survived");
    assert_contains(&raw, "- 3", "and so did the second's");
    p.expect(&["check"]);
}

/// The same wart in `depends_on`, which predates composition and had never been
/// filed.
#[test]
fn two_branches_adding_a_dependency_merge_by_union() {
    let p = repository();
    p.add("First", &[]);
    p.add("Second", &[]);
    p.add("Work", &[]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "three items"]);

    on_branch(&p, "one", "main", &["set", "4", "depends_on+=2"]);
    on_branch(&p, "two", "main", &["set", "4", "depends_on+=3"]);

    git(&p, &["checkout", "-q", "main"]);
    git(&p, &["merge", "--no-edit", "one"]);
    assert!(merge(&p, "two").ok(), "dependencies union too");

    let v: serde_json::Value =
        serde_json::from_str(&p.expect(&["show", "4", "--json"]).stdout).expect("JSON");
    let deps: Vec<u64> = v["depends_on"]
        .as_array()
        .expect("depends_on")
        .iter()
        .filter_map(serde_json::Value::as_u64)
        .collect();
    assert_eq!(deps, vec![2, 3], "both, in a stable order");
}

/// A value one side deliberately removed must not come back because the other
/// side simply did not touch it.
#[test]
fn a_removal_survives_a_merge_with_an_addition() {
    let p = repository();
    p.add("Work", &[]);
    p.expect(&["set", "2", "labels+=keep,drop"]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "two labels"]);

    on_branch(&p, "remover", "main", &["set", "2", "labels-=drop"]);
    on_branch(&p, "adder", "main", &["set", "2", "labels+=extra"]);

    git(&p, &["checkout", "-q", "main"]);
    git(&p, &["merge", "--no-edit", "remover"]);
    assert!(merge(&p, "adder").ok(), "this still has an answer");

    let raw = p.expect(&["show", "2", "--raw"]).stdout;
    assert_contains(&raw, "keep", "the untouched label");
    assert_contains(&raw, "extra", "and the added one");
    assert!(
        !raw.contains("drop"),
        "a deliberate removal came back from the dead:\n{raw}"
    );
}

/// Two people saying different things about one fact is not a merge cairn
/// should guess at. The reason to trust this driver is that it only resolves
/// what has an answer.
#[test]
fn a_disagreement_about_one_fact_is_still_a_conflict() {
    let p = repository();
    p.add("Work", &[]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "an item"]);

    on_branch(&p, "one", "main", &["set", "2", "priority=p0"]);
    on_branch(&p, "two", "main", &["set", "2", "priority=p3"]);

    git(&p, &["checkout", "-q", "main"]);
    git(&p, &["merge", "--no-edit", "one"]);
    let second = merge(&p, "two");
    assert!(
        !second.ok(),
        "a scalar disagreement has no correct resolution, so it must not be \
         resolved:\n{}",
        second.all()
    );

    let path = p
        .root()
        .join("cairn/items")
        .read_dir()
        .expect("items")
        .flatten()
        .map(|e| e.path())
        .find(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("0002"))
        })
        .expect("the contested item");
    let text = std::fs::read_to_string(&path).expect("read");
    assert!(
        text.contains("<<<<<<<"),
        "the markers must be left for a person:\n{text}"
    );
    assert!(
        text.contains("p0") && text.contains("p3"),
        "with both claims visible:\n{text}"
    );
}

/// An unchanged item must not become a diff, or a merge churns the tree and
/// `render --check` fails in CI for no reason.
#[test]
fn merging_the_same_addition_twice_is_stable() {
    let p = repository();
    p.add("First", &[]);
    p.add("Work", &[]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "two items"]);

    on_branch(&p, "one", "main", &["set", "3", "depends_on+=2"]);
    on_branch(&p, "two", "main", &["set", "3", "depends_on+=2"]);

    git(&p, &["checkout", "-q", "main"]);
    git(&p, &["merge", "--no-edit", "one"]);
    assert!(merge(&p, "two").ok(), "identical additions agree");

    let v: serde_json::Value =
        serde_json::from_str(&p.expect(&["show", "3", "--json"]).stdout).expect("JSON");
    assert_eq!(
        v["depends_on"].as_array().expect("depends_on").len(),
        1,
        "the same value must not appear twice"
    );
}

/// `key` is a documented frontmatter field, so it is queryable like every other
/// one. It was not, which meant `--columns key` printed an empty column and
/// `--filter key=v0.1` matched nothing — silently, because an unknown field is
/// simply missing rather than an error.
#[test]
fn a_key_is_queryable_like_any_other_field() {
    let p = Project::new();
    milestone(&p, "v0.9", Some("2027-01-01"));
    p.add("Work", &[]);

    assert_eq!(
        p.expect(&["list", "-A", "--ids", "--filter", "key=v0.9"])
            .lines(),
        vec!["0001".to_string()],
    );
    assert_contains(
        &p.expect(&["list", "-A", "--plain", "--columns", "id,key"])
            .stdout,
        "v0.9",
        "and shows as a column",
    );
    // An item with no key is unset rather than absent, so `key=` finds them.
    assert_eq!(
        p.expect(&["list", "-A", "--ids", "--filter", "key="])
            .lines(),
        vec!["0002".to_string()],
    );
}
// --- renumber, which rewrites everything ------------------------------------

/// `renumber` is the highest blast radius per line in the program: a bug does
/// not produce a wrong answer, it produces a backlog that no longer refers to
/// itself. Running it twice must be the same as running it once.
#[test]
fn renumber_is_idempotent() {
    let p = Project::new();
    p.add("First", &[]);
    p.add("Second", &[]);
    p.write(
        "cairn/items/0001-a-copy.md",
        "---\nid: 1\ntitle: A copy\nstatus: backlog\n---\nbody\n",
    );

    p.expect(&["renumber"]);
    let after_once: Vec<String> = p.expect(&["list", "-A", "--ids"]).lines();
    let files_once = p.files("cairn/items");

    let out = p.expect(&["renumber"]).all();
    assert_contains(
        &out,
        "no duplicate ids",
        "the second pass has nothing to do",
    );
    assert_eq!(after_once, p.expect(&["list", "-A", "--ids"]).lines());
    assert_eq!(files_once, p.files("cairn/items"), "and moved no file");
}

/// A cycle is a state the schema forbids and `check` reports. `renumber` must
/// still be able to repair the ids, because refusing would leave somebody with
/// two problems and no way to fix either.
#[test]
fn renumber_repairs_ids_even_where_the_graph_is_broken() {
    let p = Project::new();
    p.add("One", &[]);
    p.add("Two", &[]);
    // A cycle, written by hand because the commands refuse to create one.
    for (id, dep, name) in [(1u32, 2u32, "one"), (2, 1, "two")] {
        p.write(
            &format!("cairn/items/{id:04}-{name}.md"),
            &format!(
                "---\nid: {id}\ntitle: {name}\nstatus: backlog\ndepends_on:\n  - {dep}\n---\nbody\n"
            ),
        );
    }
    p.write(
        "cairn/items/0002-a-collision.md",
        "---\nid: 2\ntitle: A collision\nstatus: backlog\n---\nbody\n",
    );

    assert!(
        !p.run(&["check"]).ok(),
        "the project is broken to begin with"
    );
    let out = p.expect(&["renumber"]).all();
    assert_contains(&out, "renumbered", "");

    // The duplicate is gone even though the graph is still a cycle.
    let ids = p.expect(&["list", "-A", "--ids"]).lines();
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(ids.len(), unique.len(), "duplicate ids survived");
}

/// A file the process cannot write is the interesting failure: what matters is
/// not that it fails but that nothing is left half-done and invisible.
#[test]
#[cfg(unix)]
fn renumber_that_cannot_finish_leaves_everything_findable() {
    use std::os::unix::fs::PermissionsExt;

    let p = Project::new();
    p.add("Keeper", &[]);
    p.write(
        "cairn/items/0001-a-copy.md",
        "---\nid: 1\ntitle: A copy\nstatus: backlog\n---\nbody\n",
    );
    let before = p.expect(&["list", "-A", "--count"]).trimmed();

    let dir = p.path("cairn/items");
    let mode = std::fs::metadata(&dir).unwrap().permissions().mode();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).unwrap();
    let out = p.run(&["renumber"]);
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(mode)).unwrap();

    assert!(!out.ok(), "it should not claim success: {}", out.all());
    // Every item is still readable, and the lock is free for the next command.
    assert_eq!(
        p.expect(&["list", "-A", "--count"]).trimmed(),
        before,
        "an item went missing"
    );
    assert!(p.run(&["renumber"]).ok(), "the lock was left held");
}

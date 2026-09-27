// Worktrees of one repository, and what each can see of the others' work.
// Real `git worktree`s throughout: what matters is what git reports.
mod support;
use support::*;

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

fn in_git(p: &Project) {
    git(p, &["init", "-q", "-b", "main"]);
    commit(p, "base");
}

/// A repository whose `main` holds `n` unclaimed items.
fn repository_of(n: usize) -> Project {
    let p = Project::new();
    for k in 1..=n {
        p.add(&format!("Item {k}"), &[]);
    }
    in_git(&p);
    p
}

fn ids(out: &Out) -> Vec<String> {
    out.stdout.lines().map(str::to_string).collect()
}

#[test]
fn a_claim_in_one_worktree_is_refused_in_another() {
    let p = repository_of(1);
    let linked = worktree(&p, "feature");
    p.expect(&["claim", &p.id(1), "--as", "first"]);
    let refused = linked.fails(&["claim", &p.id(1), "--as", "second"]);
    assert_contains(
        &refused.stderr,
        "already claimed by first on main",
        "names the holder and the branch",
    );
    assert_contains(&refused.stderr, "--force", "says how to override");
    assert_eq!(
        linked.json(&["show", &p.id(1), "--json"])["assignee"],
        serde_json::Value::Null,
        "a refusal writes nothing"
    );
    linked.expect(&["claim", &p.id(1), "--as", "second", "--force"]);
    assert_eq!(
        linked.json(&["show", &p.id(1), "--json"])["assignee"],
        "second"
    );
}

#[test]
fn claim_next_takes_different_work_in_each_worktree() {
    let p = repository_of(2);
    let a = worktree(&p, "a");
    let b = worktree(&p, "b");
    let first = a.expect(&["claim", "--next", "--quiet"]).trimmed();
    let second = p.expect(&["claim", "--next", "--quiet"]).trimmed();
    assert_ne!(first, second, "the second claimer saw the first's claim");
    let third = b.run(&["claim", "--next", "--quiet"]);
    assert_eq!(third.code, 1, "nothing is left:\n{}", third.all());
    assert_contains(&third.stderr, "nothing unclaimed", "says why");
}

#[test]
fn next_leaves_out_work_held_elsewhere_and_says_so() {
    let p = repository_of(2);
    let linked = worktree(&p, "feature");
    linked.expect(&["claim", &p.id(1)]);

    let offered = ids(&p.expect(&["next", "--ids"]));
    assert_eq!(offered.len(), 1, "{offered:?}");
    assert!(p.id(2).starts_with(&offered[0]), "{offered:?}");
    assert_eq!(p.json(&["next", "--json"]).as_array().unwrap().len(), 1);
    assert_contains(
        &p.expect(&["next"]).stderr,
        "1 item(s) under way in other worktrees are not offered",
        "a short list is not mistaken for a short backlog",
    );

    // The record is this checkout's: listing and filters are unchanged.
    assert_eq!(p.count(), 2);
    assert_eq!(
        p.json(&["show", &p.id(1), "--json"])["assignee"],
        serde_json::Value::Null
    );
    assert_contains(
        &p.expect(&["show", &p.id(1)]).stdout,
        "elsewhere  feature  in progress  tester",
        "show names the branch and how it stands there",
    );
}

#[test]
fn work_finished_in_another_worktree_is_not_offered() {
    let p = repository_of(1);
    let linked = worktree(&p, "feature");
    linked.expect(&["close", &p.id(1)]);
    assert_eq!(p.expect(&["next", "--count"]).trimmed(), "0");
    assert_contains(
        &p.fails(&["claim", &p.id(1)]).stderr,
        "already done on feature",
        "a status elsewhere is named when nobody holds it",
    );
}

#[test]
fn the_protocol_server_selects_and_refuses_the_same_way() {
    let p = repository_of(2);
    let linked = worktree(&p, "feature");
    linked.expect(&["claim", &p.id(1)]);

    let next = p.mcp_call("next_items", serde_json::json!({}), None);
    let text = tool_text(&next);
    assert!(!text.contains(&p.id(1)), "held elsewhere:\n{text}");
    assert!(text.contains(&p.id(2)), "free:\n{text}");

    let held = p.mcp_call("claim_item", serde_json::json!({ "id": p.id(1) }), None);
    assert!(refused(&held), "{held}");
    assert_contains(&tool_text(&held), "on feature", "names the branch");

    let taken = p.mcp_call("claim_item", serde_json::json!({}), None);
    assert!(!refused(&taken), "{taken}");
    assert_contains(&tool_text(&taken), &p.id(2), "took the free one");
}

#[test]
fn a_worktree_that_never_touched_an_item_does_not_report_it() {
    // The assignment is in the base both branches share. Then main hands it
    // back. The linked worktree still has the old copy, assigned and all, but
    // it never changed it: that copy is history, not work.
    let p = repository_of(1);
    p.expect(&["claim", &p.id(1), "--as", "first"]);
    commit(&p, "assign before splitting");
    let linked = worktree(&p, "feature");
    p.expect(&["release", &p.id(1)]);
    commit(&p, "hand it back");

    assert_eq!(
        linked.json(&["show", &p.id(1), "--json"])["assignee"],
        "first",
        "the copies differ"
    );
    assert_eq!(p.expect(&["next", "--count"]).trimmed(), "1");
    let listing = p.json(&["worktrees", "--json"]);
    assert_eq!(listing["worktrees"][0]["items"], serde_json::json!([]));
    p.expect(&["claim", &p.id(1), "--as", "third"]);
}

#[test]
fn items_filed_in_another_worktree_are_listed_committed_or_not() {
    let p = repository_of(1);
    let linked = worktree(&p, "feature");
    let committed = linked.add("Committed there", &[]);
    commit(&linked, "file one");
    let loose = linked.add("Not yet added there", &[]);

    let doc = p.json(&["worktrees", "--json"]);
    let there = &doc["worktrees"][0];
    assert_eq!(there["branch"], "feature");
    let items = there["items"].as_array().unwrap();
    let found: Vec<(&str, bool)> = items
        .iter()
        .map(|i| (i["id"].as_str().unwrap(), i["new"].as_bool().unwrap()))
        .collect();
    assert_eq!(found.len(), 2, "{found:?}");
    for id in [&committed, &loose] {
        assert!(
            found
                .iter()
                .any(|(f, new)| f.starts_with(id.as_str()) && *new),
            "{id} in {found:?}"
        );
    }
    let text = p.expect(&["worktrees"]).stdout;
    assert_contains(&text, "feature", "branch heading");
    assert_contains(&text, "Not yet added there  (new)", "marked as filed there");
}

#[test]
fn concurrent_claims_in_separate_worktrees_never_take_the_same_item() {
    const N: usize = 6;
    let p = repository_of(N);
    let linked: Vec<Project> = (0..N).map(|k| worktree(&p, &format!("w{k}"))).collect();
    let claimed: Vec<Out> = std::thread::scope(|scope| {
        let handles: Vec<_> = linked
            .iter()
            .map(|w| scope.spawn(|| w.run(&["claim", "--next", "--quiet"])))
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut got: Vec<String> = claimed
        .iter()
        .map(|o| {
            assert!(o.ok(), "{}", o.all());
            o.trimmed()
        })
        .collect();
    got.sort();
    got.dedup();
    assert_eq!(
        got.len(),
        N,
        "every worktree took a different item: {got:?}"
    );
}

#[test]
fn a_stale_claim_elsewhere_is_offered_and_taken_over_out_loud() {
    let p = Project::with(Schema::standard().claim_stale_after(3));
    p.add("Abandoned", &[]);
    in_git(&p);
    let linked = worktree(&p, "abandoned");
    linked.expect(&["claim", &p.id(1), "--as", "gone"]);
    let file = linked.expect(&["show", &p.id(1), "--path"]).trimmed();
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(
        &file,
        text.replace(&format!("claimed: {}", today()), "claimed: 2020-01-01"),
    )
    .unwrap();

    assert_eq!(p.expect(&["next", "--count"]).trimmed(), "1");
    assert_contains(
        &p.expect(&["claim", &p.id(1)]).stderr,
        "taken over from gone on abandoned",
        "said out loud",
    );
}

#[test]
fn a_worktree_on_another_format_is_not_read_and_says_so() {
    let p = repository_of(1);
    let linked = worktree(&p, "future");
    linked.expect(&["claim", &p.id(1), "--as", "elsewhere"]);
    let config = linked
        .read("cairn.toml")
        .replace("format = 4", "format = 5");
    linked.write("cairn.toml", &config);

    p.expect(&["claim", &p.id(1), "--as", "here"]);
    assert_contains(
        &p.expect(&["worktrees"]).stderr,
        "future is on format 5, not 4, and was not read",
        "a worktree left out is named",
    );
}

#[test]
fn without_git_every_command_is_as_it_was() {
    let p = repository_of(1);
    let linked = worktree(&p, "feature");
    linked.expect(&["claim", &p.id(1), "--as", "first"]);
    let bin_only = std::path::Path::new(bin()).parent().unwrap();
    let path = bin_only.to_str().unwrap();
    let out = p.run_env(
        &["claim", &p.id(1), "--as", "second"],
        &[("PATH", Some(path))],
    );
    assert!(out.ok(), "{}", out.all());
}

#[test]
fn a_project_before_uuids_is_not_surveyed() {
    // Counted ids name different items on different branches, so a copy in
    // another worktree says nothing about the item with that number here.
    let p = format_one();
    in_git(&p);
    let linked = worktree(&p, "feature");
    let file = "cairn/items/0001-scheduled.md";
    let text = linked.read(file);
    linked.write(
        file,
        &text.replace("status: backlog", "status: doing\nassignee: other"),
    );
    assert_eq!(p.expect(&["next", "--count"]).trimmed(), "2");
}

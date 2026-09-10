// cairn — end-to-end tests: workflow.
//
// Copyright (c) 2026 Oddur Sigurdsson. MIT licensed; see LICENSE.
//
// Choosing what to do and taking it: `next`, `claim`, `search`, and the
// dependency graph that decides what is ready.
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

// --- dependencies -----------------------------------------------------------

/// A project where item 2 waits on item 1.
fn with_dependency() -> (Project, String, String) {
    let p = Project::new();
    let blocker = p.add("The blocker", &[]);
    let dependent = p.add("Waits on the blocker", &["-d", &blocker]);
    (p, blocker, dependent)
}

#[test]
fn blocked_and_ready_resolve_through_the_dependency_graph() {
    let (p, blocker, dependent) = with_dependency();
    assert_eq!(p.count_of("blocked=true"), 1);
    assert_eq!(p.count_of(&format!("id={dependent},ready=true")), 0);
    assert_eq!(p.count_of(&format!("id={blocker},ready=true")), 1);

    let listed = p
        .expect(&["list", "--columns", "id,blockers", "--plain"])
        .stdout;
    assert_contains(&listed, "1", "blockers resolve to ids");

    p.expect(&["close", &blocker, "-q"]);
    assert_eq!(
        p.count_of("blocked=true"),
        0,
        "closing the blocker unblocks"
    );
    assert_eq!(p.count_of(&format!("id={dependent},ready=true")), 1);
}

#[test]
fn a_dependency_on_a_missing_item_does_not_block() {
    // A typo elsewhere should be a `check` error, not a reason to hide work.
    let p = Project::new();
    p.write(
        "cairn/items/0001-lonely.md",
        "---\nid: 1\ntitle: Lonely\nstatus: backlog\ndepends_on: [42]\n---\nbody\n",
    );
    assert_eq!(p.count_of("blocked=true"), 0);
    p.fails(&["check"]);
}
// --- next -------------------------------------------------------------------

#[test]
fn next_hides_blocked_work_and_can_be_asked_for_it() {
    let (p, _, dependent) = with_dependency();
    let ready = p.expect(&["next", "-n", "50", "--ids"]).stdout;
    assert!(!ready.contains(&dependent), "blocked work is not offered");
    let all = p.expect(&["next", "-n", "50", "--blocked", "--ids"]).stdout;
    assert_contains(&all, &dependent, "--blocked includes it");
}

#[test]
fn next_reports_dependency_state_in_json() {
    let (p, _, _) = with_dependency();
    let rows = p.json(&["next", "-n", "1", "--json"]);
    let first = &rows.as_array().unwrap()[0];
    assert!(
        first.get("blockers").is_some(),
        "blockers travel with the item"
    );
    assert_eq!(first["ready"], true);
}

#[test]
fn next_puts_work_already_under_way_first() {
    let p = seeded();
    p.expect(&["set", "3", "status=doing", "-q"]);
    let ids = p.expect(&["next", "--ids"]).lines();
    assert_eq!(
        ids[0], "0003",
        "finishing something beats starting something"
    );
}

#[test]
fn next_respects_its_limit() {
    let p = seeded();
    assert_eq!(p.expect(&["next", "-n", "2", "--ids"]).lines().len(), 2);
}
// --- claiming ---------------------------------------------------------------

#[test]
fn claiming_assigns_the_item_and_starts_it() {
    let p = seeded();
    let id = p.expect(&["claim", "--next", "-q"]).trimmed();
    let item = p.json(&["show", &id, "--json"]);
    assert_eq!(item["assignee"], "tester");
    assert_eq!(item["category"], "active");
}

#[test]
fn an_item_someone_else_holds_is_refused() {
    let p = seeded();
    p.expect(&["claim", "1", "--as", "somebody", "-q"]);
    let out = p.fails(&["claim", "1"]);
    assert_contains(&out.all(), "already claimed by somebody", "who holds it");
    p.expect(&["claim", "1", "--force", "-q"]);
    assert_eq!(p.json(&["show", "1", "--json"])["assignee"], "tester");
}

#[test]
fn blocked_work_cannot_be_claimed_by_accident() {
    let (p, _, dependent) = with_dependency();
    let out = p.fails(&["claim", &dependent]);
    assert_contains(&out.all(), "blocked by", "what is in the way");
    p.expect(&["claim", &dependent, "--force", "-q"]);
}

#[test]
fn releasing_hands_an_item_back() {
    let p = seeded();
    p.expect(&["claim", "1", "-q"]);
    p.expect(&["release", "1", "-q"]);
    let item = p.json(&["show", "1", "--json"]);
    assert_eq!(item["assignee"], serde_json::Value::Null);
    assert_eq!(item["category"], "open");
}

#[test]
fn claim_next_skips_work_that_is_already_taken() {
    let p = seeded();
    let first = p
        .expect(&["claim", "--next", "--as", "other", "-q"])
        .trimmed();
    let second = p.expect(&["claim", "--next", "-q"]).trimmed();
    assert_ne!(first, second, "a claimed item is not offered again");
}
// --- search -----------------------------------------------------------------

#[test]
fn search_covers_titles_bodies_and_labels() {
    let p = Project::new();
    let id = p.add("Searchable", &["--body", "the needle is in this haystack"]);
    p.add("Labelled", &["-l", "distinctive-label"]);
    assert_contains(
        &p.expect(&["search", "haystack", "--ids"]).stdout,
        &id,
        "body",
    );
    assert_contains(
        &p.expect(&["search", "searchable", "--ids"]).stdout,
        &id,
        "title",
    );
    assert_eq!(
        p.expect(&["search", "distinctive-label", "--ids"])
            .lines()
            .len(),
        1,
        "labels"
    );
    assert_eq!(
        p.expect(&["search", "haystack", "--titles", "--ids"])
            .lines()
            .len(),
        0,
        "--titles skips bodies"
    );
    p.fails(&["search", "zzzznotfound"]);
}

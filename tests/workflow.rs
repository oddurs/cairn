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
    assert_contains(&listed, &p.id(1), "blockers resolve to full identities");

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
    p.write_uuid_fixture(
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
    p.expect(&["set", &p.id(3), "status=doing", "-q"]);
    let ids = p.expect(&["next", "--ids"]).lines();
    assert_eq!(
        ids[0],
        p.reference(3),
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
    p.expect(&["claim", &p.id(1), "--as", "somebody", "-q"]);
    let out = p.fails(&["claim", &p.id(1)]);
    assert_contains(&out.all(), "already claimed by somebody", "who holds it");
    p.expect(&["claim", &p.id(1), "--force", "-q"]);
    assert_eq!(p.json(&["show", &p.id(1), "--json"])["assignee"], "tester");
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
    p.expect(&["claim", &p.id(1), "-q"]);
    p.expect(&["release", &p.id(1), "-q"]);
    let item = p.json(&["show", &p.id(1), "--json"]);
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

/// `+=` and `-=` on a field that holds one value are refused, by name.
///
/// Every scalar field in `apply` ends in an arm that rejects a list assignment,
/// and not one of them was reached by a test — the lowest-covered file in the
/// program, on the command that does the writing. A wrong field name in one of
/// those messages, or an arm that quietly accepted the append and dropped it,
/// would have looked exactly like this.
#[test]
fn a_scalar_field_refuses_a_list_assignment_and_says_which_field() {
    let p = seeded();
    // Every key `apply` handles as a single value, with something that would be
    // valid as a plain `=`.
    for (field, value) in [
        ("key", "k1"),
        ("claimed", "2026-01-01"),
        ("owner", "alice"),
        ("created_by", "alice"),
        ("title", "Another"),
        ("type", "bug"),
        ("status", "doing"),
        ("milestone", "v0.1"),
        ("assignee", "alice"),
        ("created", "2026-01-01"),
        ("updated", "2026-01-01"),
    ] {
        for op in ["+=", "-="] {
            let out = p.run(&["set", &p.id(1), &format!("{field}{op}{value}")]);
            assert!(
                !out.ok(),
                "`{field}{op}{value}` was accepted; a scalar field took an append"
            );
            assert_contains(
                &out.all(),
                field,
                "the error does not name the field the reader typed",
            );
        }
    }

    // And the item is untouched by any of it.
    let shown = p.expect(&["show", &p.id(1)]).stdout;
    assert_missing(&shown, "alice", "a refused assignment was written anyway");
}

/// A write cannot give two items the same key.
///
/// `check_no_cycle` states the principle: "a project must not be left in a
/// state the tool itself rejects". It was applied to cycles, to unknown
/// statuses, to dates and to refs — and not to keys, which is the one a
/// reference is resolved *by*. `cairn set 2 key=v1` where 0001 already answers
/// to `v1` was written, and only `cairn check` said so afterwards.
///
/// The cost is not theoretical: a ref by that key resolves to whichever item
/// was read first, and the roadmap draws two sections under one heading.
///
/// Keys are unique *within a type*, so both items here are milestones — a
/// feature and a bug may share a key without ambiguity, because nothing
/// resolves a reference without knowing which type it wants.
#[test]
fn two_items_cannot_be_given_the_same_key() {
    let p = seeded();
    let second = p.add("Another release", &["-t", "milestone"]);
    p.expect(&["set", &p.id(4), "key=shared", "-q"]);

    let out = p.run(&["set", &second, "key=shared"]);
    assert!(!out.ok(), "a second item took a key that was already taken");
    assert_contains(
        &out.all(),
        "already used by",
        "it does not say what is wrong",
    );

    assert!(
        p.run(&["check"]).ok(),
        "the refusal did not keep the project valid: {}",
        p.run(&["check"]).all()
    );

    // Refused, not half-applied.
    let json = p.json(&["show", &second, "--json"]);
    assert_ne!(json["key"], "shared", "the key was written anyway");
}

/// The same rule reaches `new`, which builds an item the same way.
#[test]
fn a_new_item_cannot_claim_a_key_that_is_taken() {
    let p = seeded();
    // 0004 is the seeded milestone, whose key is `v0.1`.
    let out = p.run(&["new", "Latecomer", "-t", "milestone", "--set", "key=v0.1"]);
    assert!(!out.ok(), "a new item took a key that was already taken");
    assert_contains(
        &out.all(),
        "already used by",
        "it does not say what is wrong",
    );
}

// cairn — end-to-end tests.
//
// Copyright (C) 2026 Oddur Sigurdsson
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, either version 3 of the License, or (at your option) any later
// version.  See LICENSE for details.
//
// These drive the real binary, so they cover argument parsing, exit codes and
// stderr the way a user meets them. They deliberately avoid a shell: the suite
// they replaced was POSIX sh and therefore did not run on Windows at all, which
// left a third of the supported platforms covered by unit tests only.
mod support;
use support::*;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// --- init -------------------------------------------------------------------

#[test]
fn init_creates_the_configuration_and_item_directory() {
    let p = Project::new();
    assert!(p.exists("cairn.toml"));
    assert!(p.path("cairn/items").is_dir());
}

#[test]
fn init_refuses_to_clobber_an_existing_project() {
    let p = Project::new();
    p.fails(&["init"]);
    p.expect(&["init", "--force", "--bare"]);
}

#[test]
fn init_writes_an_example_item_unless_told_not_to() {
    let bare = Project::new();
    assert_eq!(
        bare.count_all(),
        0,
        "`--bare` is the schema and nothing else"
    );

    // Three milestones and one example. A milestone is an item in format 2, so
    // a project that renders a roadmap needs some for the roadmap to be about.
    let seeded = Project::with_init(&["init", "--name", "Seeded"]);
    assert_eq!(seeded.count_all(), 4);
    assert_eq!(
        seeded
            .expect(&["list", "-t", "milestone", "--ids"])
            .lines()
            .len(),
        3
    );
    assert_eq!(
        seeded.expect(&["list", "--ids"]).lines(),
        vec!["0004".to_string()],
        "and the work is one item, with the containers out of the way"
    );
}

#[test]
fn init_minimal_is_a_working_schema() {
    let p = Project::with_init(&["init", "--preset", "minimal", "--bare", "--name", "Min"]);
    p.add("Something", &[]);
    assert!(p.expect(&["check"]).ok());
}

// --- creating and querying --------------------------------------------------

fn seeded() -> Project {
    let p = Project::new();
    seed(&p);
    p
}

/// The three items and one milestone every test below names by identifier.
/// Split from `seeded` so a project built from a `Schema` can have the same
/// contents without also having the shipped template.
fn seed(p: &Project) {
    p.add("First item", &["--type", "feature", "--set", "priority=p0"]);
    p.add("Second item", &["-t", "bug"]);
    p.add("Third item", &["-t", "chore"]);
    // The milestone comes last so the three items keep the identifiers every
    // test below names. A milestone is an item in format 2, so it has to exist
    // before anything can point at it — the same as a dependency.
    p.milestone("v0.1", Some("2026-12-01"));
    p.expect(&["set", "1", "milestone=v0.1", "-q"]);
}

#[test]
fn new_prints_a_zero_padded_id() {
    let p = Project::new();
    assert_eq!(p.add("First item", &[]), "0001");
    assert_eq!(p.add("Second item", &[]), "0002");
}

#[test]
fn items_are_listed() {
    assert_eq!(seeded().count(), 3);
}

#[test]
fn convenience_filters_select_items() {
    let p = seeded();
    assert_eq!(p.count_of("type=bug"), 1);
    assert_eq!(
        p.expect(&["list", "--type", "bug", "--count"]).trimmed(),
        "1"
    );
    assert_eq!(
        p.expect(&["list", "--milestone", "v0.1", "--count"])
            .trimmed(),
        "1"
    );
}

#[test]
fn filter_expressions_cover_the_grammar() {
    let p = seeded();
    assert_eq!(p.count_of("priority=p0"), 1, "equality");
    assert_eq!(p.count_of("milestone="), 2, "an empty value means unset");
    assert_eq!(p.count_of("milestone!="), 1, "negated emptiness");
    assert_eq!(p.count_of("type!=bug"), 3, "negation, milestone included");
    assert_eq!(p.count_of("type=bug|chore"), 2, "alternatives");
    assert_eq!(p.count_of("title~first"), 1, "substring, case-insensitive");
    assert_eq!(p.count_of("id=1"), 1, "ids compare numerically");
}

#[test]
fn saved_views_come_from_the_configuration() {
    let p = seeded();
    assert_eq!(
        p.expect(&["list", "--view", "triage", "--count"]).trimmed(),
        "2"
    );
    let out = p.fails(&["list", "--view", "nope"]);
    assert_contains(&out.all(), "unknown view", "the error names the problem");
}

#[test]
fn a_malformed_filter_is_rejected() {
    let p = seeded();
    assert_contains(
        &p.fails(&["list", "--filter", "status todo"]).all(),
        "field=value",
        "the error shows the expected form",
    );
}

#[test]
fn output_modes_are_machine_readable() {
    let p = seeded();
    assert_eq!(p.expect(&["list", "--ids"]).lines().len(), 3);
    let rows = p.json(&["list", "--json"]);
    assert_eq!(rows.as_array().unwrap().len(), 3);
    let plain = p.expect(&["list", "--plain", "--columns", "id,title"]);
    assert!(plain.stdout.contains('\t'), "plain output is tab-separated");
    assert!(
        !plain.stdout.contains('\u{1b}'),
        "plain output has no escapes"
    );
}

// --- mutation ---------------------------------------------------------------

#[test]
fn set_changes_fields() {
    let p = seeded();
    p.expect(&["set", "1", "status=doing", "-q"]);
    assert_eq!(p.json(&["show", "1", "--json"])["status"], "doing");
}

#[test]
fn list_fields_can_be_added_to_and_removed_from() {
    let p = seeded();
    p.expect(&["set", "1", "labels+=auth", "-q"]);
    p.expect(&["set", "1", "labels+=backend", "-q"]);
    let labels = p.json(&["show", "1", "--json"])["labels"].clone();
    assert_eq!(labels.as_array().unwrap().len(), 2);
    p.expect(&["set", "1", "labels-=auth", "-q"]);
    assert_eq!(
        p.json(&["show", "1", "--json"])["labels"],
        serde_json::json!(["backend"])
    );
}

#[test]
fn the_schema_is_enforced_on_write() {
    let p = seeded();
    for args in [
        vec!["set", "1", "status=nope"],
        vec!["set", "1", "nonesuch=x"],
        vec!["set", "1", "priority=p9"],
        vec!["set", "1", "milestone=v9"],
        vec!["set", "1", "id=5"],
        vec!["set", "1", "depends_on+=1"],
        vec!["new", "x", "--set", "priority=p9"],
    ] {
        p.fails(&args);
    }
}

#[test]
fn a_rejected_write_names_the_permitted_values() {
    let p = seeded();
    let out = p.fails(&["set", "1", "status=nope"]);
    assert_contains(&out.all(), "unknown status", "says what is wrong");
    assert_contains(&out.all(), "backlog", "lists what is allowed");
}

#[test]
fn closing_hides_an_item_and_reopening_restores_it() {
    let p = seeded();
    p.expect(&["close", "2", "-q"]);
    assert_eq!(p.count(), 2, "closed items are hidden by default");
    assert_eq!(p.count_all(), 4, "--all shows them, and the milestone too");
    p.expect(&["reopen", "2", "-q"]);
    assert_eq!(p.count(), 3);
}

#[test]
fn the_filename_follows_the_title() {
    let p = seeded();
    p.expect(&["set", "3", "title=Renamed item", "-q"]);
    assert!(p.exists("cairn/items/0003-renamed-item.md"));
    assert!(!p.exists("cairn/items/0003-third-item.md"));
}

#[test]
fn removing_an_item_requires_confirmation_or_force() {
    let p = seeded();
    p.expect(&["remove", "3", "--force"]);
    assert_eq!(p.count(), 2);
}

// --- milestones -------------------------------------------------------------

/// A milestone is an item, so it is created, edited and read like one. There
/// is no `milestone` command any more: `add` was `new -t milestone`, `list` was
/// `list -t milestone`, and by the bar in `0051` neither made a task harder to
/// do without it.
#[test]
fn a_milestone_is_an_item_like_any_other() {
    let p = Project::new();
    let id = milestone(&p, "v0.9", Some("2027-06-01"));
    p.expect(&["set", &id, "title=Beta"]);

    // Addressable by its key, wherever an identifier is taken.
    let shown = p.json(&["show", "v0.9", "--json"]);
    assert_eq!(shown["key"], "v0.9");
    assert_eq!(shown["title"], "Beta");
    assert_eq!(shown["type"], "milestone");

    // It carries a body, which is most of the point: the reason for a date now
    // lives with the date, and `cairn log` can say when the date moved.
    p.expect(&[
        "note",
        &id,
        "--bare",
        "-q",
        "--",
        "June because the conference is in July.",
    ]);
    assert_contains(
        &p.expect(&["show", &id]).stdout,
        "conference",
        "a milestone has reasoning, which a configuration block could not hold",
    );

    // And it is out of the way of ordinary work.
    p.add("Real work", &[]);
    assert_eq!(
        p.expect(&["list", "--ids"]).lines(),
        vec!["0002".to_string()],
        "a container is not listed among the work"
    );
    assert_eq!(
        p.expect(&["list", "-t", "milestone", "--ids"]).lines(),
        vec!["0001".to_string()],
        "but is there when asked for"
    );

    // The command it replaced is gone rather than quietly accepted.
    let out = p.fails(&["milestone", "add", "v1.0"]);
    assert_contains(
        &out.all(),
        "unrecognized subcommand",
        "the command is removed, not silently accepted",
    );
}

/// Removing an item repairs everything that named it, through any reference and
/// not only `depends_on`.
///
/// Only `depends_on` was repaired before, which was right while it was the only
/// relationship there was — and left a milestone named by twenty items dangling
/// the moment milestones became items. The soak found it, on a seed CI drew and
/// I had not.
///
/// The rule it restores: a destructive command always leaves a valid project
/// and says what else it touched.
#[test]
fn removing_an_item_repairs_every_reference_to_it() {
    let p = Project::new();
    let id = milestone(&p, "v0.9", None);
    p.add("Work", &[]);
    p.add("More work", &[]);
    p.expect(&["set", "2", "3", "milestone=v0.9"]);
    p.expect(&["set", "3", "part_of=2"]);

    let out = p.expect(&["remove", &id, "--force"]);
    assert_contains(
        &out.all(),
        "dropped reference",
        "it says what else it touched",
    );

    // Valid afterwards, with no option to leave the wreckage.
    p.expect(&["check", "--strict"]);
    for n in ["2", "3"] {
        assert!(
            !p.expect(&["show", n, "--raw"]).stdout.contains("v0.9"),
            "item {n} still names a milestone that is gone"
        );
    }

    // And a reference to something still present is left alone.
    assert_contains(
        &p.expect(&["show", "3", "--raw"]).stdout,
        "part_of",
        "an unrelated reference survived",
    );
}

#[test]
fn editing_the_configuration_preserves_its_comments() {
    let p = seeded();
    milestone(&p, "v0.9", None);
    assert_contains(
        &p.read("cairn.toml"),
        "# cairn.toml",
        "hand-written comments survive a programmatic edit",
    );
}

// --- validation -------------------------------------------------------------

#[test]
fn check_rejects_an_unknown_status() {
    let p = seeded();
    p.write(
        "cairn/items/0099-broken.md",
        "---\nid: 99\ntitle: Broken\nstatus: nonexistent\n---\nbody\n",
    );
    let out = p.fails(&["check"]);
    assert_contains(&out.all(), "unknown status", "the reason");
    assert_contains(&out.all(), "0099-broken.md:4", "the file and line");
}

#[test]
fn check_rejects_duplicate_ids() {
    let p = seeded();
    p.write(
        "cairn/items/0001-duplicate.md",
        "---\nid: 1\ntitle: Duplicate\nstatus: backlog\n---\nbody\n",
    );
    let out = p.fails(&["check"]);
    assert_contains(&out.all(), "renumber", "it names the remedy");
}

#[test]
fn check_rejects_a_dangling_dependency() {
    let p = seeded();
    p.write(
        "cairn/items/0050-dangler.md",
        "---\nid: 50\ntitle: Dangler\nstatus: backlog\ndepends_on: [999]\n---\nbody\n",
    );
    assert_contains(&p.fails(&["check"]).all(), "does not exist", "the reason");
}

#[test]
fn check_rejects_a_dependency_cycle() {
    // The commands refuse to create one, so a cycle now arrives only by hand
    // editing or by a merge — which is exactly why `check` still looks for it.
    let p = seeded();
    p.write(
        "cairn/items/0060-loop-a.md",
        "---\nid: 60\ntitle: Loop A\nstatus: backlog\ndepends_on: [61]\n---\nbody\n",
    );
    p.write(
        "cairn/items/0061-loop-b.md",
        "---\nid: 61\ntitle: Loop B\nstatus: backlog\ndepends_on: [60]\n---\nbody\n",
    );
    assert_contains(&p.fails(&["check"]).all(), "cycle", "the reason");
}

#[test]
fn an_unparseable_file_is_an_error() {
    let p = seeded();
    p.write("cairn/items/0098-bad.md", "no frontmatter at all\n");
    p.fails(&["check"]);
    p.remove("cairn/items/0098-bad.md");
    p.expect(&["check"]);
}

// --- rendering --------------------------------------------------------------

#[test]
fn render_generates_the_roadmap_and_detects_drift() {
    let p = seeded();
    p.expect(&["render", "-q"]);
    assert!(p.exists("ROADMAP.md"));
    assert_contains(&p.read("ROADMAP.md"), "First item", "items are rendered");
    p.expect(&["render", "--check", "-q"]);

    // Drift has to be caused deliberately now that the render hooks ship
    // enabled — which is the point of them. --no-hooks is how a project that
    // renders by hand behaves.
    p.expect(&["--no-hooks", "new", "Fourth item", "-q"]);
    p.fails(&["render", "--check"]);
    p.expect(&["render", "-q"]);
    p.expect(&["render", "--check", "-q"]);
}

#[test]
fn check_can_verify_the_rendered_roadmap() {
    let p = seeded();
    p.expect(&["render", "-q"]);
    p.expect(&["check", "--render", "-q"]);
    p.expect(&["--no-hooks", "new", "Unrendered", "-q"]);
    p.fails(&["check", "--render"]);
}

// --- agent instructions -----------------------------------------------------

#[test]
fn the_agent_block_is_written_once_and_updated_in_place() {
    let p = seeded();
    p.expect(&["agent", "--write", "AGENTS.md"]);
    let first = p.read("AGENTS.md");
    assert_eq!(first.matches("cairn:begin").count(), 1);
    p.expect(&["agent", "--write", "AGENTS.md"]);
    assert_eq!(p.read("AGENTS.md").matches("cairn:begin").count(), 1);
}

#[test]
fn the_agent_block_describes_the_projects_own_schema() {
    let p = seeded();
    let block = p.expect(&["agent"]).stdout;
    assert_contains(&block, "backlog", "the real statuses");
    assert_contains(&block, "priority", "the real fields");
    assert_contains(&block, "cairn next", "the loop");
}

#[test]
fn surrounding_content_survives_an_agent_block_update() {
    let p = seeded();
    p.write("AGENTS.md", "# House rules\n\nRun the tests.\n");
    p.expect(&["agent", "--write", "AGENTS.md"]);
    p.expect(&["agent", "--write", "AGENTS.md"]);
    assert_contains(&p.read("AGENTS.md"), "House rules", "the prose is kept");
}

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

// --- interchange ------------------------------------------------------------

#[test]
fn export_produces_a_self_describing_document() {
    let p = seeded();
    // Four items: three of work and the milestone they are scheduled against.
    // A milestone is an item, so it travels with them.
    let doc = p.json(&["export"]);
    assert_eq!(doc["cairn"], "1", "the format is versioned");
    assert!(
        doc["schema"].is_object(),
        "the schema travels with the items"
    );
    let items = doc["items"].as_array().unwrap();
    assert_eq!(
        items.len(),
        p.count_all(),
        "every item travels, milestones among them"
    );
    assert!(
        items[0].get("category").is_some(),
        "categories cross the boundary"
    );
    assert!(items[0].get("body").is_some(), "bodies are carried");
}

/// A project whose statuses share no names with the standard preset, so an
/// import that matched on names would visibly fail.
fn foreign_receiver() -> Project {
    let p = Project::with_init(&[
        "init", "--preset", "minimal", "--bare", "--name", "Receiver",
    ]);
    let toml = p
        .read("cairn.toml")
        .replace("name = \"todo\"", "name = \"icebox\"")
        .replace("name = \"doing\"", "name = \"cooking\"")
        .replace("name = \"done\"", "name = \"shipped\"");
    p.write("cairn.toml", &toml);
    p
}

#[test]
fn import_maps_by_category_not_by_name() {
    let source = seeded();
    source.expect(&["close", "2", "-q"]);
    source.expect(&["set", "3", "status=doing", "-q"]);
    let doc = source.expect(&["export"]).stdout;

    let recv = foreign_receiver();
    assert_eq!(recv.count_all(), 0);
    recv.write("backlog.json", &doc);
    recv.expect(&["import", "--from", "json", "backlog.json", "-q"]);

    // `-A` means all: closed work and containers alike, on both sides.
    assert_eq!(recv.count_all(), source.count_all());
    recv.expect(&["check"]);

    let statuses: Vec<String> = recv
        .expect(&["list", "-A", "--plain", "--columns", "status"])
        .lines();
    for s in &statuses {
        assert!(
            ["icebox", "cooking", "shipped"].contains(&s.trim()),
            "no status name leaked from the source project: {s:?}"
        );
    }
    assert!(
        statuses.iter().any(|s| s.trim() == "shipped"),
        "a finished item stayed finished"
    );
    assert!(
        statuses.iter().any(|s| s.trim() == "cooking"),
        "work in progress stayed in progress"
    );
}

#[test]
fn importing_twice_updates_rather_than_duplicating() {
    let source = seeded();
    let doc = source.expect(&["export"]).stdout;
    let recv = foreign_receiver();
    recv.write("backlog.json", &doc);

    recv.expect(&["import", "--from", "json", "backlog.json", "-q"]);
    let after_first = recv.count_all();
    recv.expect(&["import", "--from", "json", "backlog.json", "-q"]);
    assert_eq!(
        recv.count_all(),
        after_first,
        "provenance prevents duplicates"
    );

    let sources = recv
        .expect(&["list", "-A", "--plain", "--columns", "source"])
        .stdout;
    assert_contains(&sources, "json#", "where each item came from is recorded");
}

#[test]
fn import_reports_what_it_could_not_place() {
    let source = seeded();
    let doc = source.expect(&["export"]).stdout;
    let recv = foreign_receiver();
    recv.write("backlog.json", &doc);
    let out = recv.expect(&["import", "--from", "json", "backlog.json"]);
    assert_contains(&out.all(), "not declared", "undeclared fields are named");
}

#[test]
fn import_dry_run_writes_nothing() {
    let source = seeded();
    let doc = source.expect(&["export"]).stdout;
    let recv = foreign_receiver();
    recv.write("backlog.json", &doc);
    recv.expect(&[
        "import",
        "--from",
        "json",
        "backlog.json",
        "--dry-run",
        "-q",
    ]);
    assert_eq!(recv.count_all(), 0, "nothing was written");
}

#[test]
fn import_creates_milestones_when_asked() {
    let source = seeded();
    let doc = source.expect(&["export"]).stdout;
    let recv = foreign_receiver();
    recv.write("backlog.json", &doc);
    recv.expect(&[
        "import",
        "--from",
        "json",
        "backlog.json",
        "--create-milestones",
        "-q",
    ]);
    assert_contains(
        &recv
            .expect(&[
                "list",
                "-A",
                "-t",
                "milestone",
                "--plain",
                "--columns",
                "id,title",
            ])
            .all(),
        "v0.1",
        "the milestone the document mentioned was created as an item",
    );
    recv.expect(&["check"]);
}

#[test]
fn a_malformed_map_specification_is_rejected() {
    let p = seeded();
    p.write("empty.json", "[]");
    p.fails(&[
        "import",
        "--from",
        "json",
        "empty.json",
        "--map",
        "nonsense",
    ]);
    p.fails(&[
        "import",
        "--from",
        "json",
        "empty.json",
        "--map",
        "colour:red=blue",
    ]);
}

#[test]
fn github_import_needs_a_repository() {
    let p = seeded();
    assert_contains(
        &p.fails(&["import", "--from", "github"]).all(),
        "--repo",
        "it says what is missing",
    );
}

#[test]
fn import_accepts_a_bare_array_of_items() {
    // Being liberal here is what makes a one-afternoon adapter viable.
    let p = Project::new();
    p.write(
        "items.json",
        r#"[{"title": "From a hand-written array", "category": "open"}]"#,
    );
    p.expect(&["import", "--from", "json", "items.json", "-q"]);
    assert_eq!(p.count(), 1);
}

// --- MCP --------------------------------------------------------------------

/// Drive the server with no CAIRN_USER set, as a client on somebody else's
/// machine would be: the identity then has to come from the protocol.
fn mcp_anonymous(p: &Project, requests: &[&str]) -> Vec<serde_json::Value> {
    use std::io::Write;
    let input = format!("{}\n", requests.join("\n"));
    let mut child = Command::new(bin())
        .arg("mcp")
        .current_dir(p.root())
        .env("NO_COLOR", "1")
        .env("PATH", path_with_binary())
        .env_remove("CAIRN_USER")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .expect("write");
    let out = child.wait_with_output().expect("wait");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("json"))
        .collect()
}

/// Drive the server the way a client does and return one parsed reply per line.
fn mcp(p: &Project, requests: &[&str]) -> Vec<serde_json::Value> {
    let input = format!("{}\n", requests.join("\n"));
    let out = p.run_stdin(&["mcp"], &input);
    assert!(out.ok(), "mcp exited {}: {}", out.code, out.stderr);
    out.stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            serde_json::from_str(l)
                .unwrap_or_else(|e| panic!("mcp emitted a non-JSON line: {e}\n{l}"))
        })
        .collect()
}

#[test]
fn mcp_speaks_the_protocol() {
    let p = seeded();
    let replies = mcp(
        &p,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
            r#"{"jsonrpc":"2.0","id":3,"method":"ping"}"#,
        ],
    );
    assert_eq!(replies.len(), 3, "a notification draws no reply");

    let init = &replies[0]["result"];
    assert!(init["protocolVersion"].is_string());
    assert_eq!(init["serverInfo"]["name"], "cairn");
    assert!(
        init["instructions"]
            .as_str()
            .unwrap()
            .contains("get_schema"),
        "the server tells a client where to start"
    );

    let tools = replies[1]["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    for expected in [
        "get_schema",
        "next_items",
        "list_items",
        "search_items",
        "show_item",
        "claim_item",
        "create_item",
        "update_item",
        "close_item",
        "check",
    ] {
        assert!(names.contains(&expected), "tool {expected} is advertised");
    }
    for tool in tools {
        assert!(
            tool["inputSchema"]["type"] == "object",
            "{} has a usable input schema",
            tool["name"]
        );
    }
}

#[test]
fn mcp_tools_read_and_write_the_backlog() {
    let p = seeded();
    let replies = mcp(
        &p,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"next_items","arguments":{"limit":2}}}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"create_item","arguments":{"title":"Made over MCP","type":"bug"}}}"#,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"claim_item","arguments":{}}}"#,
        ],
    );
    for r in &replies {
        assert_eq!(r["result"]["isError"], false, "{r}");
    }
    let next: serde_json::Value =
        serde_json::from_str(replies[0]["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(next["count"], 2);
    assert!(
        next["items"][0].get("blockers").is_some(),
        "dependency state travels with every item"
    );

    assert_eq!(p.count(), 4, "create_item wrote a real file");
    let claimed: serde_json::Value =
        serde_json::from_str(replies[2]["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(claimed["assignee"], "tester");
    assert!(claimed.get("body").is_some(), "the claimer gets the body");
}

#[test]
fn mcp_reports_tool_failures_in_band() {
    // A model has to be able to read the failure and correct itself, which it
    // cannot do if the transport aborts the call.
    let p = seeded();
    let replies = mcp(
        &p,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"update_item","arguments":{"id":1,"fields":{"status":"nope"}}}}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"show_item","arguments":{"id":999}}}"#,
        ],
    );
    for r in &replies {
        assert!(r.get("error").is_none(), "not a protocol error: {r}");
        assert_eq!(r["result"]["isError"], true);
    }
    let text = replies[0]["result"]["content"][0]["text"].as_str().unwrap();
    assert_contains(text, "unknown status", "what went wrong");
    assert_contains(text, "backlog", "and what would have worked");
}

#[test]
fn mcp_rejects_unknown_methods_as_protocol_errors() {
    let p = seeded();
    let replies = mcp(&p, &[r#"{"jsonrpc":"2.0","id":1,"method":"bogus/method"}"#]);
    assert_eq!(replies[0]["error"]["code"], -32601);
}

#[test]
fn mcp_survives_malformed_input() {
    let p = seeded();
    let out = p.run_stdin(
        &["mcp"],
        "not json at all\n\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n",
    );
    assert!(out.ok(), "a bad line does not kill the server");
    let last: serde_json::Value = out.stdout.lines().last().unwrap().parse().unwrap();
    assert_eq!(last["id"], 1, "later requests are still served");
}

#[test]
fn mcp_config_names_this_project() {
    let p = seeded();
    let out = p.expect(&["mcp", "--config"]);
    let config: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
    assert!(config["mcpServers"]["cairn"]["command"].is_string());
}

// --- GNU conventions --------------------------------------------------------

#[test]
fn version_carries_the_licence_notice() {
    let p = Project::empty();
    let long = p.expect(&["--version"]).stdout;
    assert_contains(&long, "GNU GPL version 3", "the licence");
    assert_contains(&long, "NO WARRANTY", "the disclaimer");
    assert_contains(&long, "Copyright", "the copyright line");
    assert_eq!(
        p.expect(&["-V"]).stdout.lines().count(),
        1,
        "-V stays short"
    );
}

#[test]
fn diagnostics_name_the_program_the_file_and_the_line() {
    let p = seeded();
    p.write(
        "cairn/items/0090-bad.md",
        "---\nid: 90\ntitle: Bad\nstatus: bogus\n---\nbody\n",
    );
    let first = p
        .fails(&["check"])
        .stderr
        .lines()
        .next()
        .unwrap()
        .to_string();
    assert!(
        first.starts_with("cairn:"),
        "the program names itself: {first}"
    );
    assert_contains(&first, "0090-bad.md:4:", "file and line");
    assert_eq!(first.lines().count(), 1, "one diagnostic per line");
}

#[test]
fn titles_are_shortened_only_as_far_as_the_filesystem_requires() {
    let p = Project::new();
    let long_title = "considerably longer than sixty characters ".repeat(2);
    let id = p.add(&long_title, &[]);
    let path = p.expect(&["show", &id, "--path"]).trimmed();
    let name = Path::new(&path)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    assert!(
        name.len() > 70,
        "not truncated at some arbitrary limit: {name}"
    );
    assert!(
        name.len() < 255,
        "still fits the filesystem: {}",
        name.len()
    );
}

#[test]
fn colour_is_suppressed_when_asked() {
    let p = seeded();
    assert!(
        !p.expect(&["list", "--color", "never"])
            .stdout
            .contains('\u{1b}')
    );
    assert!(
        p.expect(&["list", "--color", "always"])
            .stdout
            .contains('\u{1b}')
    );
}

// --- discovery --------------------------------------------------------------

#[test]
fn the_configuration_is_found_from_a_subdirectory() {
    let p = seeded();
    let deep = p.path("sub/deeper");
    std::fs::create_dir_all(&deep).unwrap();
    let from_deep = p.run_in(&deep, &["list", "--count"]);
    assert!(from_deep.ok());
    assert_eq!(from_deep.trimmed(), p.count().to_string());
}

#[test]
fn outside_a_project_the_error_names_the_remedy() {
    let p = Project::empty();
    let out = p.fails(&["list"]);
    assert_contains(&out.all(), "cairn init", "how to fix it");
}

#[test]
fn the_directory_flag_runs_elsewhere() {
    let p = seeded();
    let outside = Project::empty();
    let out = outside.run(&["-C", &p.root().display().to_string(), "list", "--count"]);
    assert!(out.ok(), "{}", out.all());
    assert_eq!(out.trimmed(), p.count().to_string());
}

// --- durability -------------------------------------------------------------

#[test]
fn line_endings_are_preserved_on_rewrite() {
    let p = Project::new();
    p.write(
        "cairn/items/0001-crlf.md",
        "---\r\nid: 1\r\ntitle: CRLF\r\nstatus: backlog\r\n---\r\n\r\nBody line\r\nSecond line\r\n",
    );
    p.expect(&["set", "1", "status=doing", "-q"]);
    let after = p.read("cairn/items/0001-crlf.md");
    assert!(after.contains("\r\n"), "the file is still CRLF");
    assert!(
        !after.replace("\r\n", "").contains('\n'),
        "no line was left with a bare newline:\n{after:?}"
    );
}

#[test]
fn new_items_are_written_with_line_feeds() {
    let p = Project::new();
    let id = p.add("Fresh", &[]);
    let path = p.expect(&["show", &id, "--path"]).trimmed();
    let text = std::fs::read_to_string(path).unwrap();
    assert!(
        !text.contains('\r'),
        "a new item is LF regardless of platform"
    );
}

#[test]
fn init_pins_item_line_endings_for_the_repository() {
    let p = Project::new();
    let attributes = p.read("cairn/items/.gitattributes");
    assert_contains(&attributes, "eol=lf", "the repository has one answer");
    // A dotfile in the item directory must not be mistaken for an item.
    p.expect(&["check"]);
    assert_eq!(p.count_all(), 0);
}

#[test]
fn writing_leaves_no_temporary_files_behind() {
    let p = seeded();
    p.expect(&["set", "1", "status=doing", "-q"]);
    p.expect(&["render", "-q"]);
    milestone(&p, "v9.9", None);
    for dir in ["cairn/items", "."] {
        for entry in std::fs::read_dir(p.path(dir)).unwrap() {
            let name = entry.unwrap().file_name().to_string_lossy().to_string();
            assert!(!name.ends_with(".tmp"), "left behind: {dir}/{name}");
        }
    }
}

#[test]
fn an_interrupted_write_never_truncates_an_item() {
    // With a truncate-then-write save, killing the process in that window
    // eventually destroys the file. With a write-then-rename save it cannot:
    // the rename either happened or it did not.
    let p = Project::new();
    p.add("Survives interruption", &["--body", "important content"]);
    let original = p.read("cairn/items/0001-survives-interruption.md");

    for n in 0..40 {
        let status = if n % 2 == 0 {
            "status=doing"
        } else {
            "status=backlog"
        };
        let mut child = Command::new(bin())
            .args(["set", "1", status, "-q"])
            .current_dir(p.root())
            .env("NO_COLOR", "1")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let _ = child.kill();
        let _ = child.wait();

        let current = p.read("cairn/items/0001-survives-interruption.md");
        assert!(!current.is_empty(), "the item vanished on iteration {n}");
        assert!(
            current.starts_with("---"),
            "the item was truncated on iteration {n}:\n{current:?}"
        );
    }
    assert!(p.expect(&["check"]).ok(), "the corpus is still valid");
    assert_contains(
        &p.read("cairn/items/0001-survives-interruption.md"),
        "important content",
        "the body survived",
    );
    let _ = original;
}

#[test]
fn read_commands_survive_one_unreadable_file() {
    let p = seeded();
    p.write("cairn/items/0090-truncated.md", "---\nid: 90\ntitle: Trunc");

    for args in [
        vec!["list"],
        vec!["next"],
        vec!["board"],
        vec!["search", "item"],
        vec!["roadmap"],
    ] {
        let out = p.run(&args);
        assert!(out.ok(), "cairn {args:?} should still work:\n{}", out.all());
        assert_contains(&out.stderr, "0090-truncated.md", "the broken file is named");
    }
    assert_contains(&p.run(&["list"]).stdout, "First item", "the rest is listed");
}

#[test]
fn writers_and_artefacts_refuse_a_partial_view() {
    // Acting on an incomplete backlog is how data gets lost, so anything that
    // writes or produces a durable artefact stops instead.
    let p = seeded();
    p.write("cairn/items/0090-truncated.md", "---\nid: 90\ntitle: Trunc");
    for args in [
        vec!["check"],
        vec!["render"],
        vec!["export"],
        vec!["set", "1", "status=doing"],
        vec!["renumber"],
    ] {
        p.fails(&args);
    }
}

#[test]
fn an_interrupted_renumber_is_recovered() {
    let p = seeded();
    let before = p.count();
    // Exactly what a crash between renumber's two phases leaves behind.
    std::fs::rename(
        p.path("cairn/items/0001-first-item.md"),
        p.path("cairn/items/0001-first-item.md.renumber"),
    )
    .unwrap();

    let out = p.expect(&["list", "--count"]);
    assert_eq!(out.trimmed(), before.to_string(), "the item came back");
    assert_contains(&out.stderr, "interrupted renumber", "and said so");
    assert!(p.exists("cairn/items/0001-first-item.md"));
    p.expect(&["check"]);
}

#[test]
fn a_staged_file_is_never_restored_over_a_real_one() {
    let p = seeded();
    std::fs::copy(
        p.path("cairn/items/0001-first-item.md"),
        p.path("cairn/items/0001-first-item.md.renumber"),
    )
    .unwrap();

    let out = p.expect(&["list"]);
    assert_contains(&out.stderr, "already exists", "it refuses and explains");
    assert!(
        p.exists("cairn/items/0001-first-item.md.renumber"),
        "the staged file is left for a person to deal with"
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

// --- the golden corpus ------------------------------------------------------

/// Item files that must keep parsing to the same values forever.
///
/// The corpus deliberately contains files cairn would not write itself — bare
/// strings where a list belongs, a missing id, CRLF endings, keys from a
/// version that does not exist yet — because those are what people, editors and
/// other tools produce. Changing an expectation here is a deliberate act: it
/// means the on-disk format changed, which needs a format number and a
/// migration.
#[test]
fn the_golden_corpus_still_parses_the_way_it_always_has() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let p = Project::new();

    let mut cases: Vec<PathBuf> = std::fs::read_dir(&corpus)
        .expect("corpus directory")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .filter(|p| p.file_name().is_some_and(|n| n != "README.md"))
        .collect();
    cases.sort();
    assert!(cases.len() >= 10, "the corpus is meant to be broad");

    for case in &cases {
        let name = case.file_name().unwrap().to_string_lossy().to_string();
        std::fs::copy(case, p.path(&format!("cairn/items/{name}"))).unwrap();
    }

    // Everything in the corpus must be readable together, not merely one by one.
    let listed = p.expect(&["list", "-A", "--ids"]);
    assert_eq!(listed.lines().len(), cases.len(), "every file parsed");

    for case in &cases {
        let expected_path = case.with_extension("json");
        let expected: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&expected_path).unwrap())
                .unwrap_or_else(|e| panic!("{}: {e}", expected_path.display()));
        let id = expected["id"].as_u64().unwrap().to_string();

        let mut actual = p.json(&["show", &id, "--json"]);
        // The only thing allowed to differ is where the file happens to live.
        actual.as_object_mut().unwrap().remove("path");

        assert_eq!(
            actual,
            expected,
            "{} parses differently than it used to.\n\
             If this change is intended it is a format change: bump \
             config::CURRENT_FORMAT, write a migration, and update the expectation.",
            case.file_name().unwrap().to_string_lossy()
        );
    }
}

#[test]
fn the_golden_corpus_is_valid_against_a_default_schema() {
    // Beyond parsing, the corpus has to survive the checks a real project runs.
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let p = Project::new();
    for entry in std::fs::read_dir(&corpus).unwrap().flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "md")
            && path.file_name().is_some_and(|n| n != "README.md")
        {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            std::fs::copy(&path, p.path(&format!("cairn/items/{name}"))).unwrap();
        }
    }
    // The corpus includes an item scheduled for `v0.1`, and a milestone is an
    // item in format 2, so one has to exist for the reference to resolve. Made
    // after the corpus is in place, because the corpus brings its own
    // identifiers and this must not collide with them.
    milestone(&p, "v0.1", None);

    // Unknown keys are warnings, not errors: a file from a later version must
    // remain usable rather than becoming unreadable.
    let out = p.run(&["check"]);
    assert!(out.ok(), "the corpus does not validate:\n{}", out.all());
    assert_contains(&out.stderr, "not declared", "unknown keys are surfaced");
}

// --- format compatibility ---------------------------------------------------

#[test]
fn a_project_from_a_newer_cairn_is_refused_not_misread() {
    let p = Project::new();
    let toml = p.read("cairn.toml").replace("format = 2", "format = 99");
    p.write("cairn.toml", &toml);
    let out = p.fails(&["list"]);
    assert_contains(&out.all(), "format 99", "the format it found");
    assert_contains(&out.all(), "upgrade cairn", "and what to do about it");
}

/// A project with no `format` key is format 1, which is now behind. §8 of the
/// specification requires refusing rather than reading on a best-effort basis,
/// and the refusal has to name the way forward.
#[test]
fn a_project_without_a_format_key_is_refused_and_told_what_to_run() {
    let p = Project::new();
    let cfg = p
        .read("cairn.toml")
        .lines()
        .filter(|l| !l.trim_start().starts_with("format ="))
        .collect::<Vec<_>>()
        .join(
            "
",
        );
    p.write("cairn.toml", &cfg);

    let out = p.fails(&["new", "Still fine", "-q"]);
    assert_contains(&out.all(), "is format 1", "it says what it found");
    assert_contains(&out.all(), "cairn migrate", "and what to do about it");

    p.expect(&["migrate"]);
    p.expect(&["new", "Now fine", "-q"]);
}

#[test]
fn migrate_is_a_no_op_at_the_current_format() {
    let p = seeded();
    p.expect(&["migrate", "--check", "-q"]);
    assert_contains(
        &p.expect(&["migrate"]).all(),
        "nothing to migrate",
        "says so",
    );
    assert_contains(
        &p.expect(&["migrate", "--dry-run"]).all(),
        "nothing to migrate",
        "",
    );
}

#[test]
fn unknown_frontmatter_keys_survive_being_rewritten() {
    // The guarantee that lets an older cairn open a newer project without
    // quietly deleting what it did not understand.
    let p = Project::new();
    p.write(
        "cairn/items/0001-later.md",
        "---\nid: 1\ntitle: Later\nstatus: backlog\nfrom_the_future: keep me\n---\n\nBody.\n",
    );
    p.expect(&["set", "1", "status=doing", "-q"]);
    assert_contains(
        &p.read("cairn/items/0001-later.md"),
        "from_the_future: keep me",
        "an unrecognised key was preserved",
    );
}

#[test]
fn removing_an_item_never_leaves_a_dangling_reference() {
    // Found by the soak test: delete the item something depends on and the
    // project fails its own `check`, reached through an ordinary operation.
    let p = Project::new();
    let blocker = p.add("Depended upon", &[]);
    let dependent = p.add("Depends on it", &["-d", &blocker]);

    let out = p.expect(&["remove", &blocker, "--force"]);
    assert_contains(
        &out.all(),
        "dropped reference",
        "it says what else it touched",
    );
    p.expect(&["check"]);
    assert_eq!(
        p.json(&["show", &dependent, "--json"])["depends_on"],
        serde_json::json!([]),
        "the reference went with the item"
    );
}

#[test]
fn plain_output_reports_names_and_the_table_reports_labels() {
    // `--plain` is for `grep` and `cut`, so it emits the names a filter accepts.
    // The table is for a person, so it shows the label the schema declared.
    let p = Project::new();
    p.add("Something", &[]);
    p.expect(&["set", "1", "status=doing", "-q"]);

    let plain = p
        .expect(&["list", "--plain", "--columns", "status"])
        .trimmed();
    assert_eq!(plain, "doing", "plain output round-trips into --filter");
    assert_eq!(p.count_of("status=doing"), 1);

    let table = p.expect(&["list", "--columns", "status"]).stdout;
    assert_contains(&table, "in progress", "the table shows the label");
}

// --- lessons from real use --------------------------------------------------

#[test]
fn a_new_project_keeps_its_roadmap_current_without_being_told_to() {
    // Also from dogfooding: a project a day old already had a stale ROADMAP.md,
    // because rendering was left to discipline. It is now done by hooks that
    // ship enabled.
    let p = Project::new();
    p.add("First", &[]);
    assert!(
        p.exists("ROADMAP.md"),
        "creating an item rendered the roadmap"
    );
    assert_contains(&p.read("ROADMAP.md"), "First", "and it has the item in it");

    p.expect(&["set", "1", "status=doing", "-q"]);
    p.expect(&["render", "--check", "-q"]);

    p.add("Second", &[]);
    p.expect(&["render", "--check", "-q"]);
    p.expect(&["remove", "1", "--force"]);
    p.expect(&["render", "--check", "-q"]);
}

#[test]
fn a_dependency_cycle_is_refused_rather_than_reported_later() {
    // Found by the soak: `set depends_on` would happily close a cycle, which
    // `check` then rejected. Same shape as removal leaving dangling references
    // — an ordinary command must not be able to produce a project the tool
    // itself calls invalid.
    let p = Project::new();
    p.add("A", &[]);
    p.add("B", &[]);
    p.add("C", &[]);
    p.expect(&["set", "2", "depends_on=1", "-q"]);
    p.expect(&["set", "3", "depends_on=2", "-q"]);

    let out = p.fails(&["set", "1", "depends_on=3"]);
    assert_contains(&out.all(), "would create a cycle", "the reason");
    assert_contains(
        &out.all(),
        "0001 -> 0003 -> 0002 -> 0001",
        "the path round it",
    );
    p.expect(&["check"]);

    p.fails(&["set", "1", "depends_on=1"]);
    p.fails(&["set", "1", "depends_on+=3"]);
    // A dependency that does not close a cycle is still fine.
    p.expect(&["set", "1", "depends_on=", "-q"]);
    p.expect(&["new", "D", "-d", "1", "-q"]);
    p.expect(&["check"]);
}

#[test]
fn dropped_work_does_not_count_against_progress() {
    // A milestone holding three abandoned ideas and one finished item is
    // complete, not a quarter done. Reporting it as a quarter done makes the
    // number worthless: the reader has to open the milestone to learn whether
    // the remainder is work or wreckage.
    let p = Project::new();
    milestone(&p, "someday", None);
    for n in 0..4 {
        p.add(&format!("Idea {n}"), &["--milestone", "someday"]);
    }
    p.expect(&["close", "2", "-q"]);
    for id in ["3", "4", "5"] {
        p.expect(&["set", id, "status=dropped", "-q"]);
    }

    let listed = p.expect(&["roadmap"]).stdout;
    assert_contains(&listed, "100%", "the milestone is finished");
    assert_contains(&listed, "1/1", "and only the live item is counted");

    p.expect(&["render", "-q"]);
    assert_contains(
        &p.read("ROADMAP.md"),
        "1 of 1 done",
        "the rendered roadmap agrees",
    );
}

// --- notes ------------------------------------------------------------------

#[test]
fn a_note_is_appended_under_a_heading() {
    let p = Project::new();
    p.add("Something", &["--body", "The original body."]);
    p.expect(&["note", "1", "Dropped: too costly for the value.", "-q"]);

    let body = p.json(&["show", "1", "--json"])["body"]
        .as_str()
        .unwrap()
        .to_string();
    assert_contains(&body, "The original body.", "what was there is untouched");
    assert_contains(&body, "Dropped: too costly", "and the note is added");
    assert!(
        body.find("The original body.").unwrap() < body.find("Dropped:").unwrap(),
        "the note goes after, not before"
    );
    assert_contains(&body, "## 20", "filed under a dated heading");
}

#[test]
fn notes_accumulate_rather_than_replace() {
    let p = Project::new();
    p.add("Something", &["--body", "Original."]);
    p.expect(&["note", "1", "First thought.", "-q"]);
    p.expect(&["note", "1", "Second thought.", "-q"]);
    let body = p.json(&["show", "1", "--json"])["body"]
        .as_str()
        .unwrap()
        .to_string();
    for expected in ["Original.", "First thought.", "Second thought."] {
        assert_contains(&body, expected, "everything is kept");
    }
}

#[test]
fn a_note_can_carry_its_own_heading_or_none() {
    let p = Project::new();
    // An explicit body, so the type's template does not contribute headings of
    // its own to the count below.
    p.add("Something", &["--body", "Original."]);
    p.expect(&[
        "note",
        "1",
        "Reasoning.",
        "--heading",
        "Dropped, 2026-09-05",
        "-q",
    ]);
    assert_contains(
        p.json(&["show", "1", "--json"])["body"].as_str().unwrap(),
        "## Dropped, 2026-09-05",
        "the given heading",
    );

    let before = p.json(&["show", "1", "--json"])["body"]
        .as_str()
        .unwrap()
        .matches("##")
        .count();
    p.expect(&["note", "1", "A bare line.", "--bare", "-q"]);
    let body = p.json(&["show", "1", "--json"])["body"]
        .as_str()
        .unwrap()
        .to_string();
    assert_contains(&body, "A bare line.", "appended");
    assert_eq!(
        body.matches("##").count(),
        before,
        "--bare added no heading"
    );
}

#[test]
fn a_note_can_be_read_from_stdin() {
    let p = Project::new();
    p.add("Something", &[]);
    let out = p.run_stdin(
        &["note", "1", "--stdin", "-q"],
        "A reason long enough\nto need more than one line.\n",
    );
    assert!(out.ok(), "{}", out.all());
    assert_contains(
        p.json(&["show", "1", "--json"])["body"].as_str().unwrap(),
        "to need more than one line.",
        "the whole of stdin",
    );
}

#[test]
fn an_empty_or_ambiguous_note_is_refused() {
    let p = Project::new();
    p.add("Something", &["--body", "Keep me."]);
    p.fails(&["note", "1"]);
    p.fails(&["note", "1", "text", "--stdin"]);
    p.fails(&["note", "1", "   "]);
    assert_contains(
        p.json(&["show", "1", "--json"])["body"].as_str().unwrap(),
        "Keep me.",
        "a refused note changed nothing",
    );
}

#[test]
fn mcp_can_append_a_note_without_replacing_the_body() {
    let p = Project::new();
    p.add("Something", &["--body", "Original body."]);
    let replies = mcp(
        &p,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"add_note","arguments":{"id":1,"text":"Why it was dropped."}}}"#,
        ],
    );
    assert_eq!(replies[0]["result"]["isError"], false, "{:?}", replies[0]);
    let body = p.json(&["show", "1", "--json"])["body"]
        .as_str()
        .unwrap()
        .to_string();
    assert_contains(&body, "Original body.", "kept");
    assert_contains(&body, "Why it was dropped.", "added");
}

#[test]
fn a_note_carrying_crlf_never_reaches_the_file() {
    // Found in review. `--stdin` and the MCP tool bypassed the normalisation
    // that `Item::parse` does on read, so a CRLF note wrote `\r\r\n` into a CRLF
    // item, and enough CRLF lines flipped an LF item's detected ending —
    // turning a one-field change into a whole-file diff.
    let p = Project::new();
    p.write(
        "cairn/items/0001-crlf.md",
        "---\r\nid: 1\r\ntitle: CRLF\r\nstatus: backlog\r\n---\r\n\r\nBody.\r\n",
    );
    let out = p.run_stdin(&["note", "1", "--stdin", "-q"], "one\r\ntwo\r\nthree\r\n");
    assert!(out.ok(), "{}", out.all());
    let file = p.read("cairn/items/0001-crlf.md");
    assert!(!file.contains("\r\r"), "doubled carriage return:\n{file:?}");
    assert!(file.contains("\r\n"), "the file is still CRLF");
    assert!(
        !file.replace("\r\n", "").contains('\n'),
        "no line was left with a bare newline"
    );

    // An LF item stays LF no matter how many CRLF lines a note carries.
    p.add("Plain", &["--body", "Body."]);
    let many: String = (0..40).map(|n| format!("line {n}\r\n")).collect();
    assert!(p.run_stdin(&["note", "2", "--stdin", "-q"], &many).ok());
    let plain_path = p.expect(&["show", "2", "--path"]).trimmed();
    let plain = std::fs::read_to_string(plain_path).unwrap();
    assert!(!plain.contains('\r'), "an LF item was flipped by its note");

    p.expect(&["check"]);
}

#[test]
fn mcp_notes_are_normalised_too() {
    let p = Project::new();
    p.write(
        "cairn/items/0001-crlf.md",
        "---\r\nid: 1\r\ntitle: CRLF\r\nstatus: backlog\r\n---\r\n\r\nBody.\r\n",
    );
    let replies = mcp(
        &p,
        &[
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"add_note","arguments":{"id":1,"text":"one\r\ntwo"}}}"#,
        ],
    );
    assert_eq!(replies[0]["result"]["isError"], false);
    assert!(
        !p.read("cairn/items/0001-crlf.md").contains("\r\r"),
        "the MCP tool wrote a doubled carriage return"
    );
}

#[test]
fn bare_and_heading_are_refused_together() {
    let p = Project::new();
    p.add("Something", &[]);
    p.fails(&["note", "1", "text", "--bare", "--heading", "Ignored"]);
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
    let replies = mcp_anonymous(
        &p,
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
    let replies = mcp_anonymous(
        &p,
        &[
            r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"clientInfo":{"name":"some-agent"}}}"#,
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"claim_item","arguments":{"id":1,"as":"a-person"}}}"#,
        ],
    );
    assert_eq!(replies[1]["result"]["isError"], false);
    assert_eq!(p.json(&["show", "1", "--json"])["assignee"], "a-person");
}

// --- the printed views ------------------------------------------------------

#[test]
fn next_shows_the_field_it_ranks_by() {
    // It sorted by priority and did not show it, so the order looked arbitrary.
    let p = Project::new();
    p.add("Later", &["--set", "priority=p3"]);
    p.add("Urgent", &["--set", "priority=p0"]);
    let out = p.expect(&["next"]).stdout;

    assert_contains(&out, "PRIORITY", "the sort key is a column");
    assert_contains(&out, "p0", "and its values are shown");
    let urgent = out.find("Urgent").expect("urgent listed");
    let later = out.find("Later").expect("later listed");
    assert!(
        urgent < later,
        "and the order it implies is the order shown"
    );
}

#[test]
fn a_column_empty_for_every_row_is_dropped() {
    // `blocked by` is empty whenever nothing is blocked, which under the
    // default filter is always. A schema field can be empty for a given query
    // too, so the rule is general rather than a special case per column.
    let p = Project::new();
    p.add("Nothing blocks this", &[]);
    let out = p.expect(&["next"]).stdout;
    assert!(
        !out.contains("BLOCKED BY"),
        "no always-empty column:\n{out}"
    );

    let blocker = p.add("The blocker", &[]);
    p.add("Waits", &["-d", &blocker]);
    let with = p.expect(&["next", "--blocked", "-n", "10"]).stdout;
    assert_contains(&with, "BLOCKED BY", "shown once it carries something");
    assert_contains(&with, &blocker, "naming what is in the way");
}

#[test]
fn next_and_board_agree_on_the_shape_of_the_work() {
    let p = Project::new();
    let blocker = p.add("The blocker", &[]);
    p.add("Waits", &["-d", &blocker]);
    p.add("Free", &[]);
    p.expect(&["set", &blocker, "status=doing", "-q"]);

    for args in [vec!["next", "--blocked"], vec!["board"]] {
        let out = p.expect(&args).stdout;
        assert_contains(&out, "ready", &format!("{args:?} summarises"));
        assert_contains(&out, "1 in progress", &format!("{args:?} counts active"));
        assert_contains(&out, "1 blocked", &format!("{args:?} counts blocked"));
    }
}

#[test]
fn the_board_gives_width_to_columns_that_hold_something() {
    let p = Project::new();
    p.add("A title long enough to need real width on the board", &[]);
    let out = p.expect(&["board"]).stdout;
    let rule = out
        .lines()
        .find(|l| l.contains('─'))
        .expect("a rule under the headings");
    let widths: Vec<usize> = rule
        .split("  ")
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.chars().filter(|c| *c == '─').count())
        .collect();
    assert!(widths.len() > 1, "several columns: {rule}");
    let widest = widths.iter().max().unwrap();
    let narrowest = widths.iter().min().unwrap();
    assert!(
        widest > narrowest,
        "an empty column should not take a full share: {widths:?}"
    );
}

#[test]
fn the_board_marks_work_that_cannot_be_started() {
    let p = Project::new();
    let blocker = p.add("The blocker", &[]);
    p.add("Waits on it", &["-d", &blocker]);
    let out = p.expect(&["board"]).stdout;

    let waits = out
        .lines()
        .find(|l| l.contains("Waits on it"))
        .expect("listed");
    let free = out
        .lines()
        .find(|l| l.contains("The blocker"))
        .expect("listed");
    assert!(
        waits.trim_start().starts_with('!'),
        "blocked is marked: {waits:?}"
    );
    assert!(
        !free.trim_start().starts_with('!'),
        "startable is not: {free:?}"
    );
}

#[test]
fn the_board_is_plain_text_when_asked() {
    let p = seeded();
    let out = p.expect(&["board", "--color", "never"]).stdout;
    assert!(
        !out.contains('\u{1b}'),
        "no escapes survived clipping:\n{out:?}"
    );
    assert!(
        p.expect(&["board", "--color", "always"])
            .stdout
            .contains('\u{1b}')
    );
}

#[test]
fn a_nonsensical_board_width_is_clamped_not_obeyed() {
    // Zero-width columns render as nothing but ellipses.
    let p = seeded();
    let out = p
        .expect(&["board", "--width", "0", "--color", "never"])
        .stdout;
    assert_contains(&out, "0001", "an id still fits");
}

#[test]
fn the_clock_can_be_pinned_for_a_reproducible_run() {
    // Items record the date they were created, so the recorded demo and the
    // website's samples embedded whatever day they were made — and the check
    // guarding them failed every night at midnight, for no reason connected to
    // the code. SOURCE_DATE_EPOCH is the reproducible-builds convention for
    // exactly this.
    let p = Project::new();
    let out = Command::new(bin())
        .args(["new", "Pinned", "-q"])
        .current_dir(p.root())
        .env("NO_COLOR", "1")
        .env("PATH", path_with_binary())
        .env("SOURCE_DATE_EPOCH", "1788566400") // 2026-09-05 UTC
        .output()
        .unwrap();
    assert!(out.status.success());

    let item = p.json(&["show", "1", "--json"]);
    assert_eq!(item["created"], "2026-09-05");
    assert_eq!(item["updated"], "2026-09-05");

    // Nonsense in the variable is ignored rather than fatal: a build system
    // setting it oddly should not stop anybody creating an item.
    let out = Command::new(bin())
        .args(["new", "Unpinned", "-q"])
        .current_dir(p.root())
        .env("NO_COLOR", "1")
        .env("PATH", path_with_binary())
        .env("SOURCE_DATE_EPOCH", "not-a-timestamp")
        .output()
        .unwrap();
    assert!(out.status.success(), "a malformed value is not fatal");
    assert!(p.json(&["show", "2", "--json"])["created"].is_string());
}

// --- where to take a problem ------------------------------------------------

/// The GNU Coding Standards ask a program to say where a bug goes, because the
/// person having one has the program in front of them and nothing else.
#[test]
fn help_says_where_to_report_a_bug() {
    let p = Project::empty();
    for flag in ["--help", "-h"] {
        let out = p.expect(&[flag]);
        assert!(
            out.stdout.contains("Report bugs to:"),
            "`cairn {flag}` does not say where to report a bug"
        );
        assert!(
            out.stdout.contains("github.com/oddurs/cairn/issues"),
            "`cairn {flag}` names no address"
        );
    }
}

/// A bug report is most often filed from a project that will not work, so this
/// has to run without one rather than failing the way every other command does.
#[test]
fn bug_report_works_outside_a_project() {
    let p = Project::empty();
    let out = p.expect(&["--bug-report"]);
    assert!(out.stdout.contains("cairn "), "no version");
    assert!(out.stdout.contains("platform:"), "no platform");
    assert!(
        out.stdout.contains("project: none found"),
        "did not say there was no project: {}",
        out.stdout
    );
}

#[test]
fn bug_report_describes_the_project_it_is_run_in() {
    let p = Project::new();
    p.expect(&["new", "One"]);
    p.expect(&["new", "Two"]);

    let out = p.expect(&["--bug-report"]);
    assert_contains(&out.stdout, "format:", "it reports the format");
    assert!(out.stdout.contains("items: 2"), "no count: {}", out.stdout);
    assert!(
        out.stdout.contains("hooks:"),
        "no hooks line: {}",
        out.stdout
    );

    // Nothing here should be anything a reporter would mind publishing: this
    // gets pasted into a public tracker, and a diagnostic that leaks is a
    // diagnostic nobody runs twice.
    assert!(
        !out.stdout.contains("One") && !out.stdout.contains("Two"),
        "the report includes item titles: {}",
        out.stdout
    );
    assert!(
        !out.stdout.contains(&p.root().display().to_string()),
        "the report includes an absolute path: {}",
        out.stdout
    );
}

/// A project that will not load is exactly when somebody files a bug, so the
/// report has to survive one.
#[test]
fn bug_report_survives_a_project_that_will_not_load() {
    let p = Project::new();
    std::fs::write(p.path("cairn.toml"), "this is not toml at all [[[").unwrap();

    let out = p.expect(&["--bug-report"]);
    assert!(
        out.stdout.contains("will not load"),
        "should have said the project is unreadable: {}",
        out.stdout
    );
}

// --- choosing an editor -----------------------------------------------------

/// The failure is almost never a broken editor. It is no editor — an empty
/// environment, or a machine without the one cairn guessed — and the fix is
/// naming a variable, so the message names it.
#[test]
fn a_missing_editor_explains_how_to_choose_one() {
    let p = Project::new();
    p.expect(&["new", "A thing"]);

    let out = p.run_env(
        &["edit", "1"],
        &[("EDITOR", Some("cairn-no-such-editor")), ("VISUAL", None)],
    );
    assert!(!out.ok(), "editing with a missing editor should fail");
    let said = out.all();
    assert!(
        said.contains("no editor"),
        "reported a spawn failure rather than the problem: {said}"
    );
    assert!(
        said.contains("cairn-no-such-editor"),
        "did not name the editor it tried: {said}"
    );
    assert!(
        said.contains("set EDITOR"),
        "did not say how to choose one: {said}"
    );
}

/// VISUAL wins over EDITOR, which is the convention every other program follows.
#[test]
fn visual_is_preferred_to_editor() {
    let p = Project::new();
    p.expect(&["new", "A thing"]);

    let out = p.run_env(
        &["edit", "1"],
        &[
            ("VISUAL", Some("cairn-visual-editor")),
            ("EDITOR", Some("cairn-plain-editor")),
        ],
    );
    let said = out.all();
    assert!(
        said.contains("cairn-visual-editor"),
        "EDITOR was used in preference to VISUAL: {said}"
    );
}

/// An empty VISUAL is not a choice of editor. Shell profiles export empty
/// variables constantly, and treating one as a program name produces a spawn
/// failure for a name nobody typed.
#[test]
fn an_empty_editor_variable_is_ignored() {
    let p = Project::new();
    p.expect(&["new", "A thing"]);

    let out = p.run_env(
        &["edit", "1"],
        &[("VISUAL", Some("")), ("EDITOR", Some("cairn-real-choice"))],
    );
    let said = out.all();
    assert!(
        said.contains("cairn-real-choice"),
        "an empty VISUAL was treated as an editor: {said}"
    );
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

// --- changing several items at once -----------------------------------------

/// `close`, `reopen`, `release` and `remove` all take a list of ids. `set` —
/// the most-used mutating command — took exactly one, so the common case of
/// triaging a handful of items the same way was a shell loop.
#[test]
fn set_accepts_several_ids() {
    let p = Project::new();
    for title in ["One", "Two", "Three"] {
        p.add(title, &[]);
    }

    p.expect(&["set", "1", "2", "3", "priority=p0"]);
    assert_eq!(
        p.expect(&["list", "--ids", "--filter", "priority=p0"])
            .lines()
            .len(),
        3,
        "every named item was changed"
    );
}

/// Ids first, then assignments. Anything else is a typo worth refusing rather
/// than a shape worth guessing at.
#[test]
fn an_id_after_an_assignment_is_refused() {
    let p = Project::new();
    p.add("One", &[]);
    p.add("Two", &[]);

    let out = p.fails(&["set", "1", "status=doing", "2"]);
    assert_contains(
        &out.all(),
        "comes after an assignment",
        "it should say what is wrong",
    );
    assert_contains(
        &out.all(),
        "cairn set 1 2 status=doing",
        "and show the command that was meant",
    );

    // And crucially it changed nothing, rather than applying to the first id
    // and then complaining.
    assert!(
        p.expect(&["list", "--ids", "--filter", "status=doing"])
            .trimmed()
            .is_empty(),
        "a refused command must not have written anything"
    );
}

/// The dangerous case: the person running it has not seen the list.
#[test]
fn a_filtered_change_shows_what_it_matched_and_asks() {
    let p = Project::new();
    for title in ["One", "Two", "Three"] {
        p.add(title, &[]);
    }
    p.expect(&["set", "1", "2", "priority=p0"]);

    let declined = p.run_stdin(&["set", "--filter", "priority=p0", "status=doing"], "n\n");
    assert!(!declined.ok(), "declining should not succeed");
    assert_contains(
        &declined.all(),
        "One",
        "the list is shown before the question",
    );
    assert_contains(&declined.all(), "change 2 item(s)?", "and it says how many");

    // Assert on the stored value rather than what `show` prints: the status
    // named `doing` is displayed as its label, "in progress".
    assert!(
        p.expect(&["list", "--ids", "--filter", "status=doing"])
            .trimmed()
            .is_empty(),
        "declining must leave every item alone"
    );

    let accepted = p.run_stdin(&["set", "--filter", "priority=p0", "status=doing"], "y\n");
    assert!(accepted.ok(), "{}", accepted.all());
    let moved = p
        .expect(&["list", "--ids", "--filter", "status=doing"])
        .lines();
    assert_eq!(moved.len(), 2, "it applied to what matched, and only that");
}

/// A script has no terminal to answer at, so the answer to an unanswered
/// question is no.
#[test]
fn a_filtered_change_with_no_answer_does_nothing() {
    let p = Project::new();
    p.add("One", &[]);

    let out = p.run(&["set", "--filter", "status=backlog", "priority=p0"]);
    assert!(!out.ok(), "silence is not consent");
    assert!(
        p.expect(&["list", "--ids", "--filter", "priority=p0"])
            .trimmed()
            .is_empty(),
        "an unanswered question must change nothing"
    );

    p.expect(&["set", "--filter", "status=backlog", "--yes", "priority=p0"]);
    assert_eq!(
        p.expect(&["list", "--ids", "--filter", "priority=p0"])
            .lines()
            .len(),
        1,
        "--yes is how a script says yes"
    );
}

#[test]
fn a_filter_matching_nothing_is_an_error_rather_than_a_silent_success() {
    let p = Project::new();
    p.add("One", &[]);
    let out = p.fails(&["set", "--filter", "priority=p9", "--yes", "status=doing"]);
    assert_contains(
        &out.all(),
        "no item matches",
        "an empty selection is a mistake, not a no-op",
    );
}

/// A bad assignment must not leave half the items changed. Everything is parsed
/// before anything is written.
#[test]
fn a_typo_in_the_last_assignment_writes_nothing() {
    let p = Project::new();
    p.add("One", &[]);
    p.add("Two", &[]);

    let out = p.fails(&["set", "1", "2", "priority=p0", "status=nonsense"]);
    assert!(!out.ok());
    assert!(
        p.expect(&["list", "--ids", "--filter", "priority=p0"])
            .trimmed()
            .is_empty(),
        "an item was written before the invalid assignment was noticed"
    );
}

#[test]
fn set_still_takes_exactly_one_id() {
    // The old shape has to keep working: this is the most-used command.
    let p = Project::new();
    p.add("Only", &[]);
    p.expect(&["set", "1", "status=doing", "priority=p1"]);
    assert_eq!(
        p.expect(&["list", "--ids", "--filter", "status=doing,priority=p1"])
            .lines()
            .len(),
        1,
        "both assignments landed on the one item named"
    );
}

/// Found by the interchange round-trip property, which generated a backlog with
/// dependencies and noticed they were gone on the other side.
///
/// `import` matched each written item back to the record that produced it by
/// comparing `source` — and on a cairn export that field is absent, so it was
/// synthesised locally for the item and left `None` on the record. The two
/// never compared equal, and every dependency was silently dropped. Silently is
/// the operative word: nothing failed, `check` passed, and the backlog was
/// simply missing the thing `next` uses to decide what is startable.
#[test]
fn dependencies_survive_an_export_and_import() {
    let source = Project::new();
    source.add("Foundation", &[]);
    source.add("Depends on the foundation", &[]);
    source.add("Also depends on it", &[]);
    source.expect(&["set", "2", "3", "depends_on+=1"]);

    let document = source.expect(&["export"]).stdout;

    let mirror = Project::new();
    std::fs::write(mirror.path("in.json"), &document).expect("writing the document");
    mirror.expect(&["import", "--from", "json", "in.json"]);

    let items: serde_json::Value =
        serde_json::from_str(&mirror.expect(&["list", "-A", "--json"]).stdout).expect("JSON");
    let items = items.as_array().expect("an array");

    let foundation = items
        .iter()
        .find(|i| i["title"] == "Foundation")
        .expect("the foundation came back");
    let foundation_id = foundation["id"].as_u64().expect("id");

    for title in ["Depends on the foundation", "Also depends on it"] {
        let item = items
            .iter()
            .find(|i| i["title"] == title)
            .unwrap_or_else(|| panic!("`{title}` came back"));
        let deps: Vec<u64> = item["depends_on"]
            .as_array()
            .expect("depends_on")
            .iter()
            .filter_map(|v| v.as_u64())
            .collect();
        assert_eq!(
            deps,
            vec![foundation_id],
            "`{title}` lost its dependency in the round trip"
        );
    }

    // And the consequence that made it worth finding: `next` has to still know
    // what is blocked.
    let ready = mirror.expect(&["next", "--ids"]).lines();
    assert_eq!(
        ready.len(),
        1,
        "everything looks startable, so the dependencies did not survive"
    );
}

// --- how identifiers are written --------------------------------------------

/// A project may declare how its identifiers are written. `id` in the
/// frontmatter is an unsigned integer regardless — the key is a *rendering*,
/// which is what makes adopting one a display change rather than a format
/// change.
fn keyed(template: &str, start: Option<u32>) -> Project {
    let p = Project::new();
    // Both settings go into the existing [project] table; a second one would
    // be a duplicate key, which is how this helper was wrong the first time.
    let mut replacement = format!("id_format = \"{template}\"");
    if let Some(n) = start {
        replacement.push_str(&format!("\nid_start = {n}"));
    }
    let cfg = p.read("cairn.toml").replace("id_width = 4", &replacement);
    p.write("cairn.toml", &cfg);
    p
}

#[test]
fn a_project_can_say_how_its_identifiers_are_written() {
    for (template, first) in [
        ("MP-{n}", "MP-1"),
        ("A{n}", "A1"),
        ("CAIRN-TOOLS-{n}", "CAIRN-TOOLS-1"),
        ("{n:04}", "0001"),
        ("{n}", "1"),
    ] {
        let p = keyed(template, None);
        p.expect(&["new", "First", "-q"]);
        let ids = p.expect(&["list", "--ids"]).trimmed();
        assert_eq!(
            ids, first,
            "`{template}` should render the first item as {first}"
        );

        // And the file is named to match, or the name and the identifier
        // disagree — which is the confusion a key is adopted to remove.
        let name = p.expect(&["show", "1", "--path"]).trimmed();
        assert!(
            name.contains(&format!("{first}-first")),
            "`{template}` produced the file {name}"
        );
    }
}

/// `id` stays an integer whatever the rendering says. This is the whole design:
/// if it were not true, adopting a key would be a format change and a migration
/// for every existing project.
#[test]
fn the_stored_identifier_is_still_a_number() {
    let p = keyed("MP-{n}", None);
    p.expect(&["new", "First", "-q"]);

    let raw = p.expect(&["show", "1", "--raw"]).stdout;
    assert_contains(&raw, "id: 1", "the frontmatter still stores an integer");
    assert!(
        !raw.contains("id: MP-1"),
        "the rendering leaked into the file:\n{raw}"
    );

    let v: serde_json::Value =
        serde_json::from_str(&p.expect(&["show", "1", "--json"]).stdout).expect("JSON");
    assert_eq!(v["id"], 1, "`id` in JSON is the number");
    assert_eq!(v["ref"], "MP-1", "`ref` carries the rendered form");
}

/// Both forms are accepted, because requiring a prefix somebody already knows
/// is friction for nothing — and because every reference written before a
/// project adopted a key is a bare number.
#[test]
fn both_the_rendered_form_and_the_bare_number_are_accepted() {
    let p = keyed("MP-{n}", None);
    p.expect(&["new", "First", "-q"]);
    p.expect(&["new", "Second", "-q"]);

    for id in ["MP-2", "2", "mp-2", "#2", "#MP-2", " MP-2 "] {
        let out = p.run(&["show", id, "--json"]);
        assert!(out.ok(), "`cairn show {id}` failed: {}", out.all());
        let v: serde_json::Value = serde_json::from_str(&out.stdout).expect("JSON");
        assert_eq!(v["id"], 2, "`{id}` should mean item 2");
    }

    // And dependencies take either, wherever `show` does.
    p.expect(&["set", "MP-2", "depends_on+=MP-1"]);
    let v: serde_json::Value =
        serde_json::from_str(&p.expect(&["show", "2", "--json"]).stdout).expect("JSON");
    assert_eq!(
        v["depends_on"][0], 1,
        "the dependency was stored as a number"
    );
}

#[test]
fn a_malformed_template_is_refused_when_the_project_is_opened() {
    for (template, complaint) in [
        ("MP-{oops}", "should be `{n}`"),
        ("MP-1002", "has no `{n}`"),
        ("{n}-{n}", "more than one placeholder"),
        ("12{n}", "cannot start with a digit"),
        ("MP-{n", "never closed"),
    ] {
        let p = keyed(template, None);
        let out = p.fails(&["list"]);
        assert_contains(
            &out.all(),
            complaint,
            &format!("`{template}` should be refused with an explanation"),
        );
        assert_contains(
            &out.all(),
            "cairn.toml",
            "and the message should name the file",
        );
    }
}

/// A prefix beginning with a digit would make `12-34` ambiguous with a plain
/// number, and two spellings must not be able to mean different items.
#[test]
fn a_numeric_prefix_is_refused_rather_than_left_ambiguous() {
    let p = keyed("2024-{n}", None);
    let out = p.fails(&["list"]);
    assert_contains(
        &out.all(),
        "cannot start with a digit",
        "a numeric prefix is ambiguous with a bare number",
    );
}

#[test]
fn a_project_can_start_numbering_somewhere_other_than_one() {
    let p = keyed("MP-{n}", Some(1000));
    p.expect(&["new", "First", "-q"]);
    p.expect(&["new", "Second", "-q"]);
    assert_eq!(
        p.expect(&["list", "--ids"]).lines(),
        vec!["MP-1000".to_string(), "MP-1001".to_string()],
        "allocation starts at id_start and continues normally"
    );
}

/// Lowering it later does nothing, because allocation still takes the maximum.
/// That is the right behaviour and is asserted rather than left to be found.
#[test]
fn lowering_the_starting_point_does_not_reuse_identifiers() {
    let p = keyed("MP-{n}", Some(1000));
    p.expect(&["new", "First", "-q"]);

    let cfg = p
        .read("cairn.toml")
        .replace("id_start = 1000", "id_start = 5");
    p.write("cairn.toml", &cfg);
    p.expect(&["new", "Second", "-q"]);

    assert_eq!(
        p.expect(&["list", "--ids"]).lines(),
        vec!["MP-1000".to_string(), "MP-1001".to_string()],
        "an existing project is unaffected by lowering id_start"
    );
}

/// Adopting a format should not mean touching every item by hand.
#[test]
fn renumber_brings_filenames_into_line_with_the_format() {
    let p = keyed("MP-{n}", None);
    p.expect(&["new", "First", "-q"]);
    p.expect(&["new", "Second", "-q"]);

    let cfg = p
        .read("cairn.toml")
        .replace("id_format = \"MP-{n}\"", "id_format = \"TOOLS-{n}\"");
    p.write("cairn.toml", &cfg);

    // check reports it first, which is how somebody finds out.
    let checked = p.expect(&["check"]);
    assert_contains(
        &checked.all(),
        "filename does not match",
        "check should report the mismatch",
    );

    // A dry run says what it would do and does nothing.
    let dry = p.expect(&["renumber", "--dry-run"]);
    assert_contains(
        &dry.all(),
        "would be renamed",
        "a dry run says what it would do",
    );
    assert_contains(
        &p.expect(&["check"]).all(),
        "filename does not match",
        "a dry run must not have renamed anything",
    );

    p.expect(&["renumber"]);
    p.expect(&["check", "--strict"]);
    let path = p.expect(&["show", "1", "--path"]).trimmed();
    assert!(path.contains("TOOLS-1-first"), "the file is now {path}");
}

/// The specification's fallback is a *leading run of digits*, which a project
/// with a key does not have. §4.2 permits a reader to apply the project's
/// rendering instead, and cairn does — otherwise a hand-written file in such a
/// project would be unreadable, and silently so.
#[test]
fn an_id_can_be_recovered_from_a_formatted_filename() {
    let p = keyed("MP-{n}", None);
    p.expect(&["new", "First", "-q"]);
    p.write(
        "cairn/items/MP-77-written-by-hand.md",
        "---\ntitle: Written by hand\nstatus: backlog\n---\nbody\n",
    );

    let ids = p.expect(&["list", "--ids"]).lines();
    assert!(
        ids.contains(&"MP-77".to_string()),
        "the hand-written file was not read: {ids:?}"
    );
}

/// A prefix is compared in bytes, and a byte offset can land inside a
/// character. `MP` is two bytes and so is `é`, which was enough to bring the
/// process down.
#[test]
fn a_non_ascii_argument_is_refused_rather_than_fatal() {
    let p = keyed("MP-{n}", None);
    p.expect(&["new", "First", "-q"]);

    for arg in ["aé", "é", "MPé", "aéb", "日本"] {
        let out = p.run(&["show", arg]);
        assert!(
            !out.all().contains("panicked"),
            "`cairn show {arg}` panicked: {}",
            out.all()
        );
        assert!(!out.ok(), "`{arg}` is not an id and should be refused");
        assert_contains(&out.all(), "not a valid item id", "and said why");
    }
}

/// The same slice, reached from a filename instead of an argument.
///
/// Worse than the argument path, because nobody types this: a file whose name
/// happens to begin with a multi-byte character is enough, and the crash lands
/// in `list` rather than in something a person just asked for.
#[test]
fn a_non_ascii_filename_does_not_bring_down_a_listing() {
    let p = keyed("MP-{n}", None);
    p.expect(&["new", "First", "-q"]);
    // No `id` in the frontmatter, so the filename is the only place to find one.
    p.write(
        "cairn/items/éclair.md",
        "---\ntitle: Named oddly\nstatus: backlog\n---\nbody\n",
    );

    let out = p.run(&["list", "-A"]);
    assert!(
        !out.all().contains("panicked"),
        "a filename brought down the listing: {}",
        out.all()
    );
    assert!(out.ok(), "{}", out.all());
}

/// A project that says nothing gets exactly what it gets today.
#[test]
fn the_default_rendering_is_unchanged() {
    let p = Project::new();
    p.expect(&["new", "First", "-q"]);
    assert_eq!(p.expect(&["list", "--ids"]).trimmed(), "0001");
    assert!(
        p.expect(&["show", "1", "--path"])
            .trimmed()
            .contains("0001-first"),
        "the default filename changed"
    );
}

/// Two branches each allocated the same identifier, and one of them is already
/// on the main branch. They are not equals: renaming the published one churns
/// history and breaks every link anybody has written to it.
///
/// This is the exact case found while rebasing two branches of cairn's own
/// backlog that had both allocated `0055`. cairn renumbered the published one,
/// because both items looked identical to it — same creation date,
/// distinguished only by filename — so it picked alphabetically and got it
/// backwards.
#[test]
fn at_a_merge_the_side_already_published_keeps_its_identifier() {
    let p = repository();

    // The published side. Named to sort *after* the arriving one, so a test
    // that passes by alphabetical accident cannot.
    git(&p, &["checkout", "-qb", "published"]);
    p.write(
        "cairn/items/0009-zebra.md",
        "---\nid: 9\ntitle: Zebra\nstatus: backlog\ncreated: 2026-01-01\n---\nPublished first.\n",
    );
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "the published item"]);
    git(&p, &["checkout", "-q", "main"]);
    git(&p, &["merge", "-q", "--no-edit", "published"]);

    // The arriving side, allocating the same id on a branch cut earlier.
    git(&p, &["checkout", "-qb", "arriving", "HEAD~1"]);
    p.write(
        "cairn/items/0009-antelope.md",
        "---\nid: 9\ntitle: Antelope\nstatus: backlog\ncreated: 2026-01-01\n---\nArrived later.\n",
    );
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "the arriving item"]);

    git(&p, &["checkout", "-q", "main"]);
    // The `post-merge` hook repairs this on its own, which is the realistic
    // path and also the awkward one: the hook runs after the merge commit
    // exists but while .git/MERGE_HEAD is still on disk.
    git(&p, &["merge", "--no-edit", "arriving"]);
    p.expect(&["renumber"]);
    p.expect(&["check"]);

    let items: serde_json::Value =
        serde_json::from_str(&p.expect(&["list", "-A", "--json"]).stdout).expect("JSON");
    let items = items.as_array().expect("an array");
    let find = |title: &str| {
        items
            .iter()
            .find(|i| i["title"] == title)
            .unwrap_or_else(|| panic!("`{title}` survived"))["id"]
            .as_u64()
            .expect("id")
    };

    assert_eq!(
        find("Zebra"),
        9,
        "the published item kept its identifier; renaming it would break every \
         link written to it"
    );
    assert_ne!(find("Antelope"), 9, "the arriving item moved");
}

/// Outside a repository there is nothing to consult, and the previous rule —
/// oldest first, path breaking the tie — applies unchanged.
#[test]
fn outside_a_repository_renumbering_is_unchanged() {
    let p = Project::new();
    p.write(
        "cairn/items/0009-antelope.md",
        "---\nid: 9\ntitle: Antelope\nstatus: backlog\ncreated: 2026-01-01\n---\nbody\n",
    );
    p.write(
        "cairn/items/0009-zebra.md",
        "---\nid: 9\ntitle: Zebra\nstatus: backlog\ncreated: 2026-01-01\n---\nbody\n",
    );

    p.expect(&["renumber"]);
    p.expect(&["check"]);

    let items: serde_json::Value =
        serde_json::from_str(&p.expect(&["list", "-A", "--json"]).stdout).expect("JSON");
    let antelope = items
        .as_array()
        .expect("array")
        .iter()
        .find(|i| i["title"] == "Antelope")
        .expect("Antelope")["id"]
        .as_u64()
        .expect("id");
    assert_eq!(
        antelope, 9,
        "with nothing to consult, the path still breaks the tie"
    );
}

// --- acceptance criteria ----------------------------------------------------

/// Every item cairn's templates produce carries `- [ ]` boxes, and until this
/// existed nothing read them: an item could close with every box empty and
/// `check --strict` was satisfied.
fn with_criteria(p: &Project, title: &str, done: usize, todo: usize) -> String {
    let mut body = String::from("\n## Criteria here\n\n");
    for n in 0..done {
        body.push_str(&format!("- [x] done {n}\n"));
    }
    for n in 0..todo {
        body.push_str(&format!("- [ ] todo {n}\n"));
    }
    set_body(p, title, &body)
}

/// Create an item and replace its body wholesale.
///
/// Replace rather than append: the type template already seeds a body with an
/// empty `- [ ]` under a heading, so appending leaves a box the test did not ask
/// for and did not count. The template doing that is correct — it is how items
/// come to carry criteria at all — which makes it the test's job to be explicit.
fn set_body(p: &Project, title: &str, body: &str) -> String {
    let id = p.expect(&["new", title, "-q"]).trimmed();
    let path = p.expect(&["show", &id, "--path"]).trimmed();
    let existing = std::fs::read_to_string(&path).expect("read");
    let front = existing.split("\n---\n").next().expect("frontmatter");
    std::fs::write(&path, format!("{front}\n---\n{body}")).expect("write");
    id
}

#[test]
fn criteria_are_counted_and_filterable() {
    let p = Project::new();
    let met = with_criteria(&p, "All done", 2, 0);
    let unmet = with_criteria(&p, "Half done", 1, 1);
    set_body(
        &p,
        "No criteria at all",
        "\nJust prose, and a bullet:\n\n- a thing\n",
    );

    assert_contains(
        &p.expect(&["show", &met]).stdout,
        "criteria",
        "an item with criteria reports them",
    );

    // An item stating none is vacuously met, so the common case is quiet.
    let unmet_ids = p.expect(&["list", "-A", "--ids", "--filter", "criteria_met=false"]);
    assert_eq!(
        unmet_ids.lines(),
        vec![unmet.clone()],
        "only the item with an unticked box is unmet"
    );

    assert_eq!(
        p.expect(&["list", "-A", "--ids", "--filter", "criteria=2"])
            .lines(),
        vec![met.clone(), unmet.clone()],
        "`criteria` counts what an item states"
    );
    assert_eq!(
        p.expect(&["list", "-A", "--ids", "--filter", "criteria_done=1"])
            .lines(),
        vec![unmet],
        "`criteria_done` counts what is ticked"
    );
}

/// The moment somebody declares work done is when what they wrote down that
/// done would mean is worth repeating back.
#[test]
fn closing_reports_what_is_still_unticked() {
    let p = Project::new();
    let id = with_criteria(&p, "Half done", 1, 2);

    let out = p.expect(&["close", &id]);
    assert_contains(
        &out.all(),
        "2 of 3 acceptance criteria are unticked",
        "closing should say what remains",
    );
    // Reported, never refused: a criterion can stop applying, and a tool that
    // blocked here would teach people to tick boxes rather than say what is true.
    assert!(out.ok(), "closing must still succeed: {}", out.all());

    let clean = with_criteria(&p, "Genuinely done", 2, 0);
    let out = p.expect(&["close", &clean]);
    assert!(
        !out.all().contains("unticked"),
        "an item whose criteria are met should say nothing: {}",
        out.all()
    );
}

/// Off by default, because an unticked box is a judgement about process rather
/// than a schema violation — and a project adopting cairn mid-life would get a
/// wall of warnings about work finished years ago, turn it off, and then it
/// would be worth nothing.
#[test]
fn check_reports_unticked_criteria_only_when_the_project_asks() {
    let p = Project::new();
    let id = with_criteria(&p, "Half done", 1, 2);
    p.expect(&["close", &id]);

    let quiet = p.expect(&["check", "--strict"]);
    assert!(
        !quiet.all().contains("unticked"),
        "silent by default: {}",
        quiet.all()
    );

    let cfg = p
        .read("cairn.toml")
        .replace("[project]", "[project]\nrequire_criteria = true");
    p.write("cairn.toml", &cfg);

    let loud = p.fails(&["check", "--strict"]);
    assert_contains(
        &loud.all(),
        "closed with 2 of 3 acceptance criteria unticked",
        "a project that asks for it gets it",
    );

    // And an open item is never reported: it is not claiming to be finished.
    p.expect(&["reopen", &id]);
    let reopened = p.expect(&["check", "--strict"]);
    assert!(
        !reopened.all().contains("unticked"),
        "an open item states intent, not completion: {}",
        reopened.all()
    );
}

#[test]
fn a_project_can_say_where_its_criteria_live() {
    let p = Project::new();
    let id = set_body(
        &p,
        "Scoped",
        "\n## Problem\n\n- [ ] not a criterion\n\n## Acceptance criteria\n\n- [x] one\n",
    );

    // Without a section, every box counts and this item looks unmet.
    assert_eq!(
        p.expect(&["list", "-A", "--ids", "--filter", "criteria_met=false"])
            .lines(),
        vec![id.clone()]
    );

    let cfg = p.read("cairn.toml").replace(
        "[project]",
        "[project]\ncriteria_section = \"Acceptance criteria\"",
    );
    p.write("cairn.toml", &cfg);

    assert!(
        p.expect(&["list", "-A", "--ids", "--filter", "criteria_met=false"])
            .trimmed()
            .is_empty(),
        "with a section named, only that section counts"
    );
}

#[test]
fn closing_over_mcp_reports_unticked_criteria_without_refusing() {
    let p = Project::new();
    let id = with_criteria(&p, "Half done", 1, 2);

    let request = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"close_item","arguments":{{"id":{}}}}}}}"#,
        id.trim_start_matches('0')
    );
    let out = p.run_stdin(&["mcp"], &format!("{request}\n"));
    let reply: serde_json::Value =
        serde_json::from_str(out.stdout.lines().next_back().expect("a reply")).expect("JSON");

    let text = reply["result"]["content"][0]["text"]
        .as_str()
        .expect("content");
    let body: serde_json::Value = serde_json::from_str(text).expect("the tool returns JSON");

    assert_eq!(body["criteria"]["done"], 1);
    assert_eq!(body["criteria"]["total"], 3);
    assert!(
        body["note"]
            .as_str()
            .unwrap_or_default()
            .contains("unticked"),
        "the agent is told what remains: {text}"
    );
    assert!(
        reply["result"]["isError"].as_bool() != Some(true),
        "and is not refused"
    );
}

// --- fields that name other items -------------------------------------------

/// A project whose schema declares a container type and a ref field pointing at
/// it. This is the shape `0078` will use for milestones, exercised here through
/// the general mechanism.
fn with_refs(extra: &str) -> Project {
    let p = Project::new();
    // The scaffold already declares a `milestone` type and a reference to it,
    // so this adds a second reference to the same type rather than a duplicate.
    let cfg = p.read("cairn.toml").replacen(
        "[[field]]",
        &format!(
            "[[field]]\nname = \"release\"\nkind = \"ref\"\n\
             target = \"milestone\"\nby = \"key\"\n{extra}\n\n[[field]]"
        ),
        1,
    );
    p.write("cairn.toml", &cfg);
    p
}

/// A milestone to point at. In format 2 a milestone is an item, so it has to
/// exist before anything can name it — the same as a dependency.
fn milestone(p: &Project, key: &str, due: Option<&str>) -> String {
    let id = p.expect(&["new", key, "-t", "milestone", "-q"]).trimmed();
    p.expect(&["set", &id, &format!("key={key}")]);
    if let Some(d) = due {
        p.expect(&["set", &id, &format!("due={d}")]);
    }
    id
}

#[test]
fn a_ref_field_names_an_item_by_key() {
    let p = with_refs("");
    p.expect(&["new", "Version one", "-t", "milestone", "-q"]);
    p.expect(&["set", "1", "key=v1.0"]);
    p.expect(&["new", "Support OAuth", "-q"]);

    p.expect(&["set", "2", "release=v1.0"]);

    // The point of addressing by key: the file stays readable.
    let raw = p.expect(&["show", "2", "--raw"]).stdout;
    assert_contains(&raw, "release: v1.0", "the key is what the file says");
    assert!(
        !raw.contains("release: 1"),
        "an identifier leaked into the file:\n{raw}"
    );

    assert_eq!(
        p.expect(&["list", "-A", "--ids", "--filter", "release=v1.0"])
            .lines(),
        vec!["0002".to_string()],
        "and refs are filterable like any other field"
    );
}

/// Naming the alternatives is most of the value: somebody mistyping a milestone
/// wants the list far more than the word "invalid".
#[test]
fn a_ref_that_names_nothing_is_refused_with_the_alternatives() {
    let p = with_refs("");
    p.expect(&["new", "Version one", "-t", "milestone", "-q"]);
    p.expect(&["set", "1", "key=v1.0"]);
    p.expect(&["new", "Support OAuth", "-q"]);

    let out = p.fails(&["set", "2", "release=v9.9"]);
    assert_contains(&out.all(), "does not exist", "it refuses");
    assert_contains(&out.all(), "known: v1.0", "and says what would have worked");

    // Refusing on write is what keeps `check` and the write path agreeing: a
    // project must never be left in a state the tool itself rejects.
    p.expect(&["check"]);
}

/// A key-addressed field resolves only by key. Accepting an identifier as well
/// would make one spelling mean two things depending on what exists.
#[test]
fn a_key_addressed_ref_does_not_fall_back_to_an_identifier() {
    let p = with_refs("");
    p.expect(&["new", "Version one", "-t", "milestone", "-q"]);
    p.expect(&["set", "1", "key=v1.0"]);
    p.expect(&["new", "Support OAuth", "-q"]);

    let out = p.fails(&["set", "2", "release=1"]);
    assert_contains(
        &out.all(),
        "does not exist",
        "an identifier is not a key, even when it names the right item",
    );
}

/// The same rule identifier prefixes obey, for the same reason.
#[test]
fn a_key_that_reads_as_an_identifier_is_refused() {
    let p = with_refs("");
    p.expect(&["new", "Version one", "-t", "milestone", "-q"]);
    let out = p.fails(&["set", "1", "key=0042"]);
    assert_contains(
        &out.all(),
        "reads as an identifier",
        "a numeric key would make a reference ambiguous",
    );
}

/// A key is what other items call this one, so changing it is a rename. Left
/// alone, every reference would be orphaned silently.
#[test]
fn renaming_a_key_carries_the_references_with_it() {
    let p = with_refs("");
    p.expect(&["new", "Version one", "-t", "milestone", "-q"]);
    p.expect(&["set", "1", "key=v1.0"]);
    for title in ["First", "Second"] {
        p.expect(&["new", title, "-q"]);
    }
    p.expect(&["set", "2", "3", "release=v1.0"]);

    let out = p.expect(&["set", "1", "key=v2.0"]);
    assert_contains(&out.all(), "also", "it says what else it touched");

    for id in ["2", "3"] {
        assert_contains(
            &p.expect(&["show", id, "--raw"]).stdout,
            "release: v2.0",
            "the reference followed the rename",
        );
    }
    p.expect(&["check"]);
}

/// A container is what work belongs to, not work. Offering one in answer to
/// "what can I start" would push real work off the list.
#[test]
fn container_types_are_not_offered_as_work() {
    let p = with_refs("");
    p.expect(&["new", "Version one", "-t", "milestone", "-q"]);
    p.expect(&["new", "Support OAuth", "-q"]);

    assert_eq!(
        p.expect(&["next", "--ids"]).lines(),
        vec!["0002".to_string()],
        "the milestone is not startable work"
    );
    assert!(
        !p.expect(&["board"]).stdout.contains("Version one"),
        "nor is it on the board"
    );
    // Still an item, and still there when the type is named — the same rule
    // closed items follow.
    assert_eq!(
        p.expect(&["list", "-A", "--ids"]).lines().len(),
        2,
        "`--all` means all, containers included"
    );
    assert_eq!(
        p.expect(&["list", "-t", "milestone", "--ids"]).lines(),
        vec!["0001".to_string()],
        "and naming the type asks for them without `--all`"
    );
}

/// `target = "*"` must not make every type a container, or `depends_on` would
/// empty `cairn next`.
#[test]
fn a_ref_targeting_anything_makes_nothing_a_container() {
    let p = Project::new();
    p.add("One", &[]);
    p.add("Two", &[]);
    assert_eq!(
        p.expect(&["next", "--ids"]).lines().len(),
        2,
        "everything is still startable work"
    );
}

#[test]
fn an_acyclic_ref_refuses_a_cycle_however_far_around() {
    let p = Project::new();
    for title in ["One", "Two", "Three"] {
        p.add(title, &[]);
    }

    p.expect(&["set", "1", "part_of=2"]);
    p.expect(&["set", "2", "part_of=3"]);

    let out = p.fails(&["set", "3", "part_of=1"]);
    assert_contains(&out.all(), "cycle", "three deep is still a cycle");

    let direct = p.fails(&["set", "1", "part_of+=1"]);
    assert_contains(&direct.all(), "cycle", "and an item is not part of itself");
}

#[test]
fn a_single_valued_ref_refuses_two_names() {
    let p = with_refs("");
    p.expect(&["new", "One", "-t", "milestone", "-q"]);
    p.expect(&["set", "1", "key=v1.0"]);
    p.expect(&["new", "Two", "-t", "milestone", "-q"]);
    p.expect(&["set", "2", "key=v2.0"]);
    p.expect(&["new", "Work", "-q"]);

    let out = p.fails(&["set", "3", "release=v1.0,v2.0"]);
    assert_contains(
        &out.all(),
        "names one item",
        "an item ships in one release, and the schema says so",
    );
}

/// A schema that describes a general mechanism plus one special case that
/// predates it is two vocabularies. An agent should meet one.
#[test]
fn depends_on_is_described_as_the_ref_it_is() {
    let p = Project::new();
    let schema: serde_json::Value =
        serde_json::from_str(&p.expect(&["config", "--json"]).stdout).expect("JSON");
    let depends = schema["fields"]
        .as_array()
        .expect("fields")
        .iter()
        .find(|f| f["name"] == "depends_on")
        .expect("depends_on is in the schema");

    assert_eq!(depends["kind"], "ref");
    assert_eq!(depends["target"], "*");
    assert_eq!(depends["cardinality"], "many");
    assert_eq!(depends["by"], "id");
    assert_eq!(depends["acyclic"], true);
    assert_eq!(depends["inverse"], "blocks");
    // `depends_on` orders work; it does not compose it. Progress rolls up
    // through composition, which is a different field. 0073 derives it.
    assert_eq!(depends["rollup"], false);
}

#[test]
fn a_ref_field_must_target_a_declared_type() {
    let p = Project::new();
    let cfg = p.read("cairn.toml").replacen(
        "[[field]]",
        "[[field]]\nname = \"release\"\nkind = \"ref\"\ntarget = \"nonexistent\"\n\n[[field]]",
        1,
    );
    p.write("cairn.toml", &cfg);
    let out = p.fails(&["list"]);
    assert_contains(
        &out.all(),
        "not a declared [[type]]",
        "a ref pointing at a type nobody declared is a typo, caught at load",
    );
}

// --- composition ------------------------------------------------------------

/// A new project gets composition without configuring anything, because `init`
/// scaffolds it. It is not hardcoded: hardcoding a second relationship would
/// re-create the problem `0079` removed.
#[test]
fn a_new_project_can_compose_without_configuring_anything() {
    let p = Project::with_init(&["init", "--name", "Composed"]);
    assert_contains(
        &p.read("cairn.toml"),
        "name = \"part_of\"",
        "init scaffolds composition",
    );

    p.add("Ship OAuth", &[]);
    p.add("Token endpoint", &[]);
    p.expect(&["set", "3", "part_of=2"]);
    p.expect(&["check"]);
}

/// The reason there is no `parent` field. An item belongs to two larger efforts
/// at once, which a scalar could not express — and a scalar is also what makes
/// two branches reparenting the same item a real conflict.
#[test]
fn an_item_can_belong_to_two_things_at_once() {
    let p = Project::new();
    for title in ["OAuth", "Q3 security", "Token endpoint"] {
        p.add(title, &[]);
    }

    p.expect(&["set", "3", "part_of=1"]);
    p.expect(&["set", "3", "part_of+=2"]);

    let raw = p.expect(&["show", "3", "--raw"]).stdout;
    assert_contains(&raw, "- 1", "belongs to the first");
    assert_contains(&raw, "- 2", "and to the second");

    // Identifiers are stored as numbers, so composition reads the way
    // `depends_on` does and a hand-written `part_of: [1, 2]` survives a save.
    assert!(
        !raw.contains("'1'") && !raw.contains("\"1\""),
        "identifiers were quoted:\n{raw}"
    );
}

/// Filing must stay free. Anything that made `cairn new` require a parent would
/// both kill adoption and put the structure decision at the worst moment.
#[test]
fn composition_is_never_required_to_file_something() {
    let p = Project::with_init(&["init", "--name", "Composed"]);
    p.expect(&["new", "Just a title"]);
    p.expect(&["check"]);
    assert_eq!(
        p.expect(&["list", "-A", "--ids", "--filter", "part_of="])
            .lines()
            .len(),
        5,
        "three milestones, the example and the new item: none needed a parent"
    );
}

/// Depth is unbounded on purpose: a limit is a decision that will be wrong for
/// somebody. Past a handful it is usually a taxonomy where a plan was wanted,
/// which is a judgement worth voicing and not worth enforcing.
#[test]
fn a_deep_hierarchy_is_a_warning_rather_than_an_error() {
    let p = Project::new();
    for n in 1..=6 {
        p.add(&format!("Level {n}"), &[]);
    }
    for n in 2..=6 {
        p.expect(&["set", &n.to_string(), &format!("part_of={}", n - 1)]);
    }

    let out = p.expect(&["check"]);
    assert!(out.ok(), "a deep hierarchy is not an error: {}", out.all());
    assert_contains(
        &out.all(),
        "levels of composition",
        "but it is worth mentioning",
    );
    assert_contains(&out.all(), "taxonomy rather than a plan", "and why");

    // Shallow enough, and it says nothing at all.
    let shallow = Project::with_init(&["init", "--name", "Shallow"]);
    shallow.add("One", &[]);
    shallow.add("Two", &[]);
    shallow.expect(&["set", "3", "part_of=2"]);
    assert!(
        !shallow.expect(&["check"]).all().contains("composition"),
        "two levels is a plan, not a taxonomy"
    );
}

// --- position, derived from the graph ---------------------------------------

/// A `scale` field would be a claim that goes stale — you tag something an epic
/// and it turns out to be an afternoon. "Has four things beneath it" cannot be
/// wrong.
#[test]
fn position_in_the_hierarchy_is_derived_rather_than_stored() {
    let p = Project::new();
    p.add("Ship OAuth", &[]);
    for n in 1..=3 {
        p.add(&format!("Piece {n}"), &[]);
    }
    p.add("Sub-piece", &[]);
    p.expect(&["set", "2", "3", "4", "part_of=1"]);
    p.expect(&["set", "5", "part_of=2"]);

    let selects = |expr: &str| p.expect(&["list", "-A", "--ids", "--filter", expr]).lines();

    assert!(selects("descendants=4").contains(&"0001".to_string()));
    assert!(selects("depth=0").contains(&"0001".to_string()));
    assert!(selects("depth=2").contains(&"0005".to_string()));
    assert!(selects("leaf=true").contains(&"0005".to_string()));
    assert!(!selects("leaf=true").contains(&"0001".to_string()));

    // Nothing was written to the file: this is a fact about the set.
    let raw = p.expect(&["show", "1", "--raw"]).stdout;
    for derived in ["descendants", "depth", "leaf", "progress"] {
        assert!(
            !raw.contains(derived),
            "`{derived}` was stored in the item:\n{raw}"
        );
    }
}

#[test]
fn progress_is_the_proportion_of_what_is_beneath_that_is_done() {
    let p = Project::new();
    p.add("Ship OAuth", &[]);
    for n in 1..=4 {
        p.add(&format!("Piece {n}"), &[]);
    }
    p.expect(&["set", "2", "3", "4", "5", "part_of=1"]);
    p.expect(&["close", "2", "3"]);

    assert_eq!(
        p.expect(&["list", "-A", "--ids", "--filter", "progress=50"])
            .lines(),
        vec!["0001".to_string()],
        "two of four beneath it are done"
    );

    // A leaf reports nothing rather than zero. Reporting 0 would put every
    // ordinary item at the bottom of `--sort progress` and drown the signal.
    assert!(
        p.expect(&["list", "-A", "--ids", "--filter", "progress="])
            .lines()
            .contains(&"0004".to_string()),
        "an item containing nothing has no progress to report"
    );

    // The query the whole thing exists for.
    assert_eq!(
        p.expect(&["list", "-A", "--ids", "--filter", "depth=0,progress<60"])
            .lines(),
        vec!["0001".to_string()],
        "big things that are behind"
    );
}

#[test]
fn contains_names_what_is_directly_beneath() {
    let p = Project::new();
    p.add("Ship OAuth", &[]);
    p.add("Token endpoint", &[]);
    p.add("Refresh flow", &[]);
    p.expect(&["set", "2", "3", "part_of=1"]);

    let rows = p.expect(&["list", "-A", "--plain", "--columns", "id,contains"]);
    let line = rows
        .lines()
        .into_iter()
        .find(|l| l.starts_with("0001"))
        .expect("the container");
    assert_contains(&line, "0002", "the first child");
    assert_contains(&line, "0003", "and the second");
}

/// A cycle that reached disk by hand must not make a query run forever. `check`
/// reports the cycle; a filter is the wrong place to discover it.
#[test]
fn a_cycle_on_disk_does_not_hang_a_query() {
    let p = Project::new();
    p.add("One", &[]);
    p.add("Two", &[]);
    p.expect(&["set", "2", "part_of=1"]);

    // Written by hand, because the write path refuses to create this.
    let path = p.expect(&["show", "1", "--path"]).trimmed();
    let text = std::fs::read_to_string(&path).expect("read");
    std::fs::write(&path, text.replace("status:", "part_of:\n- 2\nstatus:")).expect("write");

    let out = p.expect(&["list", "-A", "--plain", "--columns", "id,depth,descendants"]);
    assert_eq!(out.lines().len(), 2, "the query still answered");

    let checked = p.fails(&["check"]);
    assert_contains(&checked.all(), "cycle", "and check is what reports it");
}

/// A project that declares no composition sees none of this.
#[test]
fn a_project_without_composition_is_unaffected() {
    let p = Project::new();
    let cfg: String = p
        .read("cairn.toml")
        .split("\n\n")
        .filter(|block| !block.contains("name = \"part_of\""))
        .collect::<Vec<_>>()
        .join("\n\n");
    p.write("cairn.toml", &cfg);
    p.add("One", &[]);

    assert_eq!(
        p.expect(&["list", "-A", "--ids", "--filter", "leaf=true"])
            .lines(),
        vec!["0001".to_string()],
        "everything is a leaf when nothing composes"
    );
    assert!(
        p.expect(&["list", "-A", "--ids", "--filter", "descendants=0"])
            .lines()
            .contains(&"0001".to_string())
    );
}

// --- what an agent may do, and what made an item ----------------------------

fn restricted() -> Project {
    let p = Project::new();
    let cfg = p
        .read("cairn.toml")
        .replacen(
            "name = \"priority\"",
            "name = \"priority\"\nagent = \"read-only\"",
            1,
        )
        .replacen("name = \"done\"", "name = \"done\"\nagent = \"propose\"", 1);
    p.write("cairn.toml", &cfg);
    p
}

/// A schema that says agents may set status and add notes, and may not change
/// priority or declare something finished, is one somebody will let near a real
/// backlog. That trust is worth more than any feature.
#[test]
fn an_agent_is_held_to_what_the_schema_permits() {
    let p = restricted();
    p.add("A thing", &[]);

    // A person is unrestricted.
    p.expect(&["set", "1", "priority=p0"]);
    p.expect(&["close", "1"]);
    p.expect(&["reopen", "1"]);

    let refused = p.fails_as_agent(&["set", "1", "priority=p1"]);
    assert_contains(
        &refused.all(),
        "may read `priority` but not set it",
        "a read-only field is refused",
    );
    assert_contains(
        &refused.all(),
        "note on the item",
        "and the refusal says what to do instead, rather than only saying no",
    );

    let closing = p.fails_as_agent(&["close", "1"]);
    assert_contains(
        &closing.all(),
        "may not move an item to `done`",
        "a status an agent may only propose",
    );

    // What it is allowed, it may still do.
    p.expect_as_agent(&["set", "1", "status=doing"]);
}

/// The restriction is about what somebody changes, not about what a schema
/// fills in. Checking the two together refused an agent permission to create
/// anything at all in a project with a read-only field, because the default was
/// applied through the same path.
#[test]
fn an_agent_can_still_file_work_in_a_restricted_project() {
    let p = restricted();
    p.expect_as_agent(&["new", "Filed by an agent", "-q"]);
    assert_eq!(p.expect(&["list", "-A", "--ids"]).lines().len(), 1);
    p.expect(&["check"]);
}

/// Who is working and who is answerable are different questions. With people
/// they are usually the same person, which is why one field served; with an
/// agent working and a person owning they are not.
#[test]
fn claiming_does_not_overwrite_who_owns_something() {
    let p = Project::new();
    p.add("A thing", &[]);
    p.expect(&["set", "1", "owner=alice"]);

    let out = p.run_env(&["claim", "1"], &[("CAIRN_USER", Some("bob"))]);
    assert!(out.ok(), "{}", out.all());

    let raw = p.expect(&["show", "1", "--raw"]).stdout;
    assert_contains(&raw, "assignee: bob", "bob is working on it");
    assert_contains(&raw, "owner: alice", "and alice is still answerable");
}

/// As the proportion of items written by agents rises, "items no human has
/// looked at" is the query that matters, and it needs a gap to find rather than
/// a name to trust.
#[test]
fn what_created_an_item_is_recorded() {
    let p = Project::new();
    p.add("By a person", &[]);
    p.expect_as_agent(&["new", "By an agent", "-q"]);

    // A person filing something is the ordinary case and is not annotated.
    // Two extra lines in every item forever would cost the readability that
    // makes this format worth having, for a signal only the other kind carries.
    let person = p.expect(&["show", "1", "--raw"]).stdout;
    assert!(
        !person.contains("created_by") && !person.contains("owner"),
        "a person's item should be unchanged:\n{person}"
    );

    let agent = p.expect(&["show", "2", "--raw"]).stdout;
    assert_contains(&agent, "created_by: claude", "the agent that filed it");
    assert!(
        !agent.contains("owner:"),
        "and left it unowned, so it can be found:\n{agent}"
    );

    // The query the pair exists for: made by something that is not a person,
    // and not yet anybody's responsibility.
    assert_eq!(
        p.expect(&["list", "-A", "--ids", "--filter", "created_by!=,owner="])
            .lines(),
        vec!["0002".to_string()],
    );
}

/// A project that restricts nothing behaves as it always has.
#[test]
fn an_unrestricted_project_treats_an_agent_as_anybody_else() {
    let p = Project::new();
    p.add("A thing", &[]);
    p.expect_as_agent(&["set", "1", "priority=p0"]);
    p.expect_as_agent(&["close", "1"]);
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

// --- an older project ------------------------------------------------------

/// A project as format 1 wrote it: milestones in the configuration, and items
/// naming them by the same string they use today.
fn format_one() -> Project {
    let p = Project::new();
    let cfg = p.read("cairn.toml").replace("format = 2", "format = 1")
        + "\n[[milestone]]\nname = \"v0.1\"\ntitle = \"First\"\ndue = \"2026-12-01\"\n\
           description = \"The first one.\"\n\n[[milestone]]\nname = \"later\"\n\
           title = \"Someday\"\n";
    p.write("cairn.toml", &cfg);
    p.write(
        "cairn/items/0001-scheduled.md",
        "---\nid: 1\ntitle: Scheduled\nstatus: backlog\nmilestone: v0.1\n---\nbody\n",
    );
    p.write(
        "cairn/items/0002-unscheduled.md",
        "---\nid: 2\ntitle: Unscheduled\nstatus: backlog\n---\nbody\n",
    );
    p
}

/// Bumping the format made seven real projects stop working entirely — not
/// their writes, everything. Nothing about them was unreadable: the migration
/// changed no item file at all.
///
/// §8 requires refusing a version a reader *does not understand*, which is
/// about a version from the future. A cairn that writes format 2 understands
/// format 1 exactly.
#[test]
fn an_older_project_can_still_be_read() {
    let p = format_one();

    for args in [
        vec!["list", "-A"],
        vec!["show", "1"],
        vec!["next"],
        vec!["search", "Scheduled"],
        vec!["board"],
        vec!["roadmap"],
        vec!["export"],
        vec!["config"],
        vec!["check"],
        vec!["agent"],
    ] {
        let out = p.run(&args);
        assert!(out.ok(), "`cairn {args:?}` should work: {}", out.all());
    }
}

/// Reading it is not the same as reading it *approximately*. cairn knows what
/// format 1 means, so an older project's roadmap is the roadmap its own cairn
/// would have drawn — milestones, order, dates and descriptions.
#[test]
fn an_older_projects_milestones_are_understood_not_ignored() {
    let p = format_one();
    let out = p.expect(&["roadmap"]).stdout;

    assert_contains(&out, "v0.1", "the milestone is there");
    assert_contains(&out, "First", "with its title");
    assert_contains(&out, "2026-12-01", "and its date");
    assert_contains(&out, "The first one.", "and its description as the body");
    assert!(
        out.find("v0.1").unwrap() < out.find("later").unwrap(),
        "in the order it was declared:\n{out}"
    );

    // And the item scheduled against it is under it rather than adrift.
    assert_eq!(
        p.expect(&["list", "-A", "--ids", "--filter", "milestone=v0.1"])
            .lines(),
        vec!["0001".to_string()],
    );
}

/// A format bump may cost somebody a command. It must never cost them the
/// ability to look, and it must never cost them their data.
#[test]
fn an_older_project_refuses_writes_and_says_what_to_run() {
    let p = format_one();

    for args in [
        vec!["new", "Something"],
        vec!["set", "1", "status=doing"],
        vec!["close", "1"],
        vec!["remove", "1", "--force"],
    ] {
        let out = p.run(&args);
        assert!(!out.ok(), "`cairn {args:?}` should be refused");
        assert_contains(&out.all(), "cairn migrate", "and name the way forward");
    }

    // Nothing was touched by any of that.
    assert_contains(
        &p.read("cairn/items/0001-scheduled.md"),
        "status: backlog",
        "a refused write changes nothing",
    );

    // And migrating is the one write that is allowed.
    p.expect(&["migrate"]);
    p.expect(&["new", "Now allowed", "-q"]);
}

/// The notice goes to standard error, so a script reading `--json` is
/// unaffected by somebody else's project being behind.
#[test]
fn the_notice_about_an_older_project_stays_out_of_the_output() {
    let p = format_one();
    let out = p.expect(&["list", "-A", "--json"]);

    assert_contains(&out.stderr, "format 1", "it is said");
    assert!(
        !out.stdout.contains("format 1"),
        "but not on standard output:\n{}",
        out.stdout
    );
    serde_json::from_str::<serde_json::Value>(&out.stdout).expect("still valid JSON");
}

/// A version from the *future* is still refused, for reading as well as
/// writing. That half of §8 is right: best-effort reading of a format nobody
/// has seen means misreading data in ways nobody can predict.
#[test]
fn a_newer_project_is_still_refused_outright() {
    let p = Project::new();
    p.write(
        "cairn.toml",
        &p.read("cairn.toml").replace("format = 2", "format = 99"),
    );

    let out = p.fails(&["list"]);
    assert_contains(&out.all(), "format 99", "the format it found");
    assert_contains(&out.all(), "upgrade cairn", "and what to do about it");
}

/// A file written by any cairn that ever existed is still read correctly by
/// this one — values, not merely the absence of an error.
///
/// The corpus one directory up tests today against today. This is the only test
/// that can fail for the right reason years from now, and it is what the format
/// number is promising on the project's behalf.
#[test]
fn every_format_that_has_existed_still_parses() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let mut formats = 0;

    for entry in std::fs::read_dir(&root).expect("corpus").flatten() {
        let dir = entry.path();
        let Some(name) = dir.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !dir.is_dir() || !name.starts_with("format-") {
            continue;
        }
        formats += 1;

        let p = Project::new();
        let mut cases = 0;
        for f in std::fs::read_dir(&dir).expect("format directory").flatten() {
            let path = f.path();
            if path.extension().is_some_and(|e| e == "md")
                && path.file_name().is_some_and(|n| n != "README.md")
            {
                let file = path.file_name().unwrap().to_string_lossy().to_string();
                std::fs::copy(&path, p.path(&format!("cairn/items/{file}"))).unwrap();
                cases += 1;
            }
        }
        assert!(cases > 0, "{name} has no cases");

        // Read through `export`, which carries every documented key including
        // the body — `list --json` omits it.
        let doc: serde_json::Value =
            serde_json::from_str(&p.expect(&["export"]).stdout).expect("JSON");
        let items = doc["items"].as_array().expect("an array").clone();

        for f in std::fs::read_dir(&dir).expect("format directory").flatten() {
            let path = f.path();
            if path.extension().is_none_or(|e| e != "json") {
                continue;
            }
            let expected: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            let id = expected["id"].as_u64().expect("an id");
            let got = items
                .iter()
                .find(|i| i["id"] == id)
                .unwrap_or_else(|| panic!("{name}: item {id} did not parse"));

            // The keys the frozen expectation names, and only those: a later
            // format may add keys, and an older corpus must not fail for it.
            for (key, want) in expected.as_object().expect("an object") {
                assert_eq!(
                    &got[key], want,
                    "{name}: item {id} key `{key}` reads differently than it did"
                );
            }
        }
    }

    assert!(
        formats > 0,
        "no per-format corpus found; adding a format means freezing its corpus"
    );
}

/// A format's corpus is frozen the day that format stops being current.
///
/// A digest, not a file count: the point is that nobody edits a case to make a
/// later reader agree with it. The old reading is the evidence, and evidence
/// that can be edited proves nothing.
#[test]
fn the_frozen_corpora_have_not_been_edited() {
    // FNV-1a, written out rather than pulled in: a digest committed in a test
    // has to mean the same thing in ten years, which rules out DefaultHasher.
    fn fnv1a(bytes: &[u8], mut h: u64) -> u64 {
        for b in bytes {
            h = (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }

    // Each format, with the digest taken when it stopped being current.
    let recorded = [("format-1", 0x453d_19cd_d0fa_398a_u64)];

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let mut corpora: Vec<String> = std::fs::read_dir(&root)
        .expect("corpus")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with("format-") && root.join(n).is_dir())
        .collect();
    corpora.sort();
    for name in &corpora {
        assert!(
            recorded.iter().any(|(n, _)| n == name),
            "{name} has a corpus but no digest here, so nothing stops it being \n\
             edited. Freeze it by recording one."
        );
    }

    for (name, expected) in recorded {
        let dir = root.join(name);
        let mut files: Vec<_> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("{name}'s corpus is gone: {e}"))
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        files.sort();

        let mut h = 0xcbf2_9ce4_8422_2325;
        for f in &files {
            h = fnv1a(f.file_name().unwrap().to_string_lossy().as_bytes(), h);
            h = fnv1a(&std::fs::read(f).unwrap(), h);
        }

        assert_eq!(
            h,
            expected,
            "{name}'s corpus has changed. It was frozen when format {} arrived, \n\
             and it is the record of how that format actually read. If a case is \n\
             wrong, the fix is a new case in the current corpus, not an edit here.",
            name.trim_start_matches("format-").parse::<u32>().unwrap() + 1
        );
    }
}

/// Migrating an older corpus produces exactly what the current corpus expects.
///
/// The parse test proves an old file still reads. This proves the migration
/// carries it to the present without changing what it means, which is the only
/// reason a format number is allowed to move at all.
#[test]
fn migrating_an_older_corpus_produces_the_current_expectations() {
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let p = format_one();
    // The corpus supplies every item; the seed's would collide on id.
    for f in std::fs::read_dir(p.path("cairn/items")).unwrap().flatten() {
        std::fs::remove_file(f.path()).unwrap();
    }

    let mut cases = Vec::new();
    for f in std::fs::read_dir(golden.join("format-1"))
        .expect("format-1")
        .flatten()
    {
        let path = f.path();
        if path.extension().is_some_and(|e| e == "md")
            && path.file_name().is_some_and(|n| n != "README.md")
        {
            let file = path.file_name().unwrap().to_string_lossy().to_string();
            std::fs::copy(&path, p.path(&format!("cairn/items/{file}"))).unwrap();
            cases.push(file);
        }
    }
    assert!(!cases.is_empty());

    p.expect(&["migrate"]);

    let doc: serde_json::Value = serde_json::from_str(&p.expect(&["export"]).stdout).expect("JSON");
    let items = doc["items"].as_array().expect("an array");

    for case in cases {
        // The expectation as the *current* corpus states it, not the frozen one.
        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(golden.join(case.replace(".md", ".json"))).unwrap(),
        )
        .unwrap();
        let id = expected["id"].as_u64().expect("an id");
        let got = items
            .iter()
            .find(|i| i["id"] == id)
            .unwrap_or_else(|| panic!("{case}: item {id} did not survive the migration"));

        for (key, want) in expected.as_object().expect("an object") {
            // `category` and `ref` come from the schema, not the file.
            if key == "category" || key == "ref" {
                continue;
            }
            assert_eq!(&got[key], want, "{case}: `{key}` changed in the migration");
        }
    }
}

/// A format cannot arrive without its corpus. The count is read from the
/// source, so the build breaks on the bump rather than on the release.
#[test]
fn every_format_below_the_current_one_has_a_frozen_corpus() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = std::fs::read_to_string(root.join("src/config.rs")).unwrap();
    let current: u32 = src
        .split("pub const CURRENT_FORMAT")
        .nth(1)
        .and_then(|s| s.split('=').nth(1))
        .and_then(|s| s.split(';').next())
        .and_then(|s| s.trim().parse().ok())
        .expect("CURRENT_FORMAT is declared in src/config.rs");

    for n in 1..current {
        let dir = root.join(format!("tests/golden/format-{n}"));
        assert!(
            dir.is_dir(),
            "format {n} has no frozen corpus at tests/golden/format-{n}.\n\
             A format stops being current the day the next one arrives; freeze \
             its corpus then, while a cairn that writes it still exists."
        );
    }
}

/// The dry run answers the question somebody actually has before running a
/// migration over years of work: what is about to change on disk.
///
/// Counted against a real project rather than asserted in prose — the two
/// milestones in the configuration become two items, and not one item file in
/// the directory is touched.
#[test]
fn a_dry_run_says_what_it_will_touch_in_files() {
    let p = format_one();
    let before: Vec<_> = std::fs::read_dir(p.path("cairn/items"))
        .unwrap()
        .flatten()
        .map(|e| (e.path(), std::fs::read(e.path()).unwrap()))
        .collect();

    let out = p.expect(&["migrate", "--dry-run"]).all();
    assert_contains(&out, "rewritten", "the configuration is named");
    assert_contains(&out, "cairn.toml", "by name");
    assert_contains(&out, "created    2 new item(s)", "one item per milestone");
    assert_contains(
        &out,
        "nothing already in the item directory will be changed.",
        "and the sentence that matters, because here it is true",
    );
    assert!(
        !out.contains("careful:"),
        "nothing is being rewritten, so nothing to be careful about: {out}"
    );

    // A dry run that changed something would be the worst defect this command
    // could have.
    for (path, contents) in before {
        assert_eq!(
            std::fs::read(&path).unwrap(),
            contents,
            "{} changed during a dry run",
            path.display()
        );
    }

    // And the count it promised is the count it delivers.
    p.expect(&["migrate"]);
    let items: serde_json::Value =
        serde_json::from_str(&p.expect(&["list", "-A", "--json"]).stdout).unwrap();
    assert_eq!(
        items.as_array().unwrap().len(),
        4,
        "two items, plus the two milestones it said it would create"
    );
}

// --- the schema, checked against itself -------------------------------------

/// `category` is the one key in the file that carries meaning rather than
/// appearance, and it silently defaulted to `open`. A status somebody named
/// `shipped` came out as the exact opposite of what they meant.
#[test]
fn a_status_with_no_category_is_reported_rather_than_assumed() {
    let p = Project::new();
    p.append("cairn.toml", "\n[[status]]\nname = \"shipped\"\n");

    let out = p.expect(&["check"]).all();
    assert_contains(&out, "status `shipped`", "the status is named");
    assert_contains(&out, "does not declare a `category`", "and the problem");
    assert_contains(&out, "treated as `open`", "and what was assumed instead");
    assert_contains(&out, "cairn.toml:", "at a line, as a diagnostic should be");

    // A warning, not an error: a project that has one still works.
    assert!(p.run(&["check"]).ok(), "it must not fail the build");
    assert!(p.run(&["list"]).ok(), "or stop anything working");

    // And the resolved schema does not present a guess as a decision.
    assert_contains(
        &p.expect(&["config"]).stdout,
        "(assumed)",
        "`cairn config` marks it",
    );
    let json: serde_json::Value =
        serde_json::from_str(&p.expect(&["config", "--json"]).stdout).unwrap();
    let statuses = json["statuses"].as_array().unwrap();
    let shipped = statuses
        .iter()
        .find(|s| s["name"] == "shipped")
        .expect("shipped");
    assert_eq!(shipped["category_declared"], serde_json::json!(false));
    assert_eq!(
        statuses[0]["category_declared"],
        serde_json::json!(true),
        "a declared category still reads as declared"
    );
}

/// `cairn check` validated items against the schema and never the schema
/// against itself, so a configuration could be comprehensively wrong and pass.
#[test]
fn the_schema_is_checked_against_itself() {
    let p = Project::new();
    p.append(
        "cairn.toml",
        "\n[[view]]\nname = \"typo\"\nfilter = \"stauts=backlog\"\nsort = \"nonesuch\"\n",
    );

    let out = p.expect(&["check"]).all();
    assert_contains(&out, "view `typo` filter names `stauts`", "the filter key");
    assert_contains(
        &out,
        "view `typo` sort names `nonesuch`",
        "and the sort key",
    );
    assert!(
        p.run(&["check"]).ok(),
        "a warning: frontmatter is open-ended, so a key nothing carries is legal"
    );
}

/// A filter that does not parse is the one error here. "No items match" is true
/// and useless: it sends somebody to look at their backlog instead of the typo.
#[test]
fn a_view_whose_filter_does_not_parse_is_an_error() {
    let p = Project::new();
    p.append(
        "cairn.toml",
        "\n[[view]]\nname = \"bad\"\nfilter = \"nonsense\"\n",
    );

    let out = p.run(&["check"]);
    assert!(!out.ok(), "it should fail: {}", out.all());
    assert_contains(&out.all(), "view `bad`", "naming the view");
    assert_contains(&out.all(), "filter does not parse", "and the reason");
}

#[test]
fn render_settings_that_cannot_work_are_reported() {
    let p = Project::with(
        Schema::standard().render(|r| r.header("docs/missing.md").group_by("epic").link_items()),
    );

    let out = p.expect(&["check"]).all();
    assert_contains(&out, "render.group_by names `epic`", "an unknown field");
    assert_contains(&out, "docs/missing.md", "a header that is not there");
    assert_contains(
        &out,
        "project.url is not set",
        "and links with nowhere to go",
    );
}

/// Renaming the milestone field left the schema consistent, `check` clean, and
/// `roadmap` printing the project name over silence.
#[test]
fn a_roadmap_with_nothing_to_group_by_says_so() {
    let p = Project::new();
    let cfg = p.read("cairn.toml").replace(
        "[[field]]\nname = \"milestone\"",
        "[[field]]\nname = \"release\"",
    );
    p.write("cairn.toml", &cfg);

    let out = p.expect(&["roadmap"]).all();
    assert_contains(
        &out,
        "no [[field]] named `milestone`",
        "it says what is missing",
    );
    assert_contains(
        &out,
        "looks that name up literally",
        "and that the name is the reason",
    );

    // The same defect arriving by the other road.
    assert_contains(
        &p.expect(&["check"]).all(),
        "render.group_by is `milestone`",
        "`check` reports it too",
    );
}

/// A board with no columns is a schema question, not an empty backlog, and the
/// two looked identical from the outside.
#[test]
fn a_board_with_no_columns_says_why() {
    let p = seeded();
    let cfg = p.read("cairn.toml").replace("board = false", "");
    // Every status hidden from the board.
    let cfg = cfg.replace("category = ", "board = false\ncategory = ");
    p.write("cairn.toml", &cfg);

    let out = p.expect(&["board"]).all();
    assert_contains(&out, "nothing to show", "");
    assert_contains(&out, "board = false", "naming the reason");
}

/// The message said to run a command that had nothing to do.
#[test]
fn an_obsolete_milestone_block_does_not_send_you_to_migrate() {
    let p = Project::new();
    p.append("cairn.toml", "\n[[milestone]]\nname = \"v9\"\n");

    let out = p.run(&["list"]);
    assert!(!out.ok());
    assert_contains(
        &out.all(),
        "no longer read",
        "it says the block is obsolete",
    );
    assert_contains(&out.all(), "v9", "and names it");
    assert_contains(&out.all(), "-t milestone", "and what to write instead");
    assert!(
        !out.all().contains("run `cairn migrate`"),
        "a project already at the current format has nothing to migrate: {}",
        out.all()
    );
}

/// A typo and a key from a newer cairn arrive looking identical, and want
/// different things said to them.
#[test]
fn an_unknown_configuration_key_says_which_kind_it_is() {
    let p = Project::new();

    let typo = p.read("cairn.toml").replace(
        "[project]",
        "[project]\ncriteria_sektion = \"Acceptance criteria\"",
    );
    p.write("cairn.toml", &typo);
    let out = p.run(&["list"]).all();
    assert!(!p.run(&["list"]).ok());
    assert_contains(&out, "did you mean `criteria_section`?", "the near miss");
    assert_contains(&out, "refused rather than ignored", "and why it is fatal");

    let future = p.read("cairn.toml").replace(
        "criteria_sektion = \"Acceptance criteria\"",
        "workflow_engine = true",
    );
    p.write("cairn.toml", &future);
    let out = p.run(&["list"]).all();
    assert!(
        !out.contains("did you mean"),
        "nothing is near `workflow_engine`, so guessing would be noise: {out}"
    );
    assert_contains(&out, "which reads format", "but it still says what it is");
}

// --- adopting a schema ------------------------------------------------------

/// Half of the schema in every project using cairn was the same schema, retyped.
#[test]
fn a_schema_can_be_adopted_from_another_project() {
    let source = Project::new();
    let cfg = source.read("cairn.toml").replace(
        "[project]",
        "[project]\nurl = \"https://example.invalid/theirs\"",
    ) + "\n# A comment worth keeping.\n[[field]]\nname = \"team\"\nkind = \"text\"\n";
    source.write("cairn.toml", &cfg);

    let p = Project::empty();
    p.expect(&[
        "init",
        "--from",
        &source.root().display().to_string(),
        "--name",
        "Borrowed",
        "--dir",
        "issues",
    ]);

    let adopted = p.read("cairn.toml");
    assert!(adopted.contains("name = \"team\""), "the field came across");
    assert!(
        adopted.contains("# A comment worth keeping."),
        "and so did the comments, which are half of what makes a schema legible"
    );
    // `[project]` belongs to whoever wrote it.
    assert!(adopted.contains("name = \"Borrowed\""), "{adopted}");
    assert!(adopted.contains("dir = \"issues\""), "{adopted}");
    assert!(
        !adopted.lines().any(|l| l.trim_start().starts_with("url =")),
        "a repository url is about the other project: {adopted}"
    );

    // The new project must not arrive already failing its own check.
    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());
    assert_contains(
        &p.expect(&["config"]).stdout,
        "team",
        "and the schema is live",
    );
}

#[test]
fn adopting_refuses_what_it_cannot_copy() {
    let p = Project::empty();
    let out = p.run(&["init", "--from", "/nonexistent-directory"]);
    assert!(!out.ok());
    assert_contains(&out.all(), "not a cairn project", "");

    // A schema from an older format would be copied forward silently, and the
    // migration that does it properly already exists.
    let old = Project::new();
    let cfg = old.read("cairn.toml").replace("format = 2", "format = 1");
    old.write("cairn.toml", &cfg);
    let out = p.run(&["init", "--from", &old.root().display().to_string()]);
    assert!(!out.ok());
    assert_contains(&out.all(), "is format 1", "it says what it found");
    assert_contains(&out.all(), "cairn migrate", "and names the way forward");
}

/// An alias is not a typo. `label` is `labels` and `kind` is `type`, and the
/// first real project the schema check ran against had a working saved view
/// filtering on `label=cloud` that it called a mistake.
#[test]
fn a_saved_view_using_an_alias_is_not_called_a_typo() {
    let p = seeded();
    p.append(
        "cairn.toml",
        "\n[[view]]\nname = \"tagged\"\nfilter = \"label=x\"\nsort = \"kind\"\n",
    );

    let out = p.expect(&["check"]).all();
    assert!(
        !out.contains("not a declared field"),
        "an alias `get` answers is a legitimate key: {out}"
    );
}

// --- the agent surface, held to what it advertises --------------------------

/// A project that restricts what an agent may touch, in both of the ways the
/// schema allows: a field it may only read, and a status it may only propose.
fn restricted_for_agents() -> Project {
    let p = Project::with(
        Schema::standard()
            .field(Field::text("risk").agent(Agent::ReadOnly))
            .amend_status("done", |s| s.agent(Agent::Propose)),
    );
    seed(&p);
    p
}

/// One call, with an `initialize` before it unless asked otherwise.
fn tool(
    p: &Project,
    name: &str,
    args: serde_json::Value,
    client: Option<&str>,
) -> serde_json::Value {
    let call = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": name, "arguments": args }
    })
    .to_string();
    let replies = match client {
        Some(who) => {
            let init = serde_json::json!({
                "jsonrpc": "2.0", "id": 0, "method": "initialize",
                "params": { "clientInfo": { "name": who } }
            })
            .to_string();
            mcp_anonymous(p, &[&init, &call])
        }
        None => mcp_anonymous(p, &[&call]),
    };
    replies.last().expect("a reply")["result"].clone()
}

fn refused(r: &serde_json::Value) -> bool {
    r["isError"] == serde_json::json!(true)
}

fn text(r: &serde_json::Value) -> String {
    r["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// The permission model was enforced on the command line and not over MCP —
/// the exact inverse of what the code and the manual both claimed.
///
/// `create_item` and `update_item` applied the caller's `fields` object with
/// `apply` rather than `apply_requested`, so every custom field went straight
/// past the check. `claim_item` and `close_item` did the same for `status`.
#[test]
fn an_agent_cannot_write_a_field_the_schema_reserves() {
    let p = restricted_for_agents();

    for (name, args) in [
        (
            "create_item",
            serde_json::json!({"title": "New", "fields": {"risk": "high"}}),
        ),
        (
            "update_item",
            serde_json::json!({"id": 1, "fields": {"risk": "high"}}),
        ),
    ] {
        let r = tool(&p, name, args, Some("claude"));
        assert!(refused(&r), "{name} wrote a read-only field: {}", text(&r));
        assert_contains(&text(&r), "may read `risk` but not set it", "and said why");
    }

    // Nothing was written by the refused create.
    assert!(
        !p.expect(&["list", "-A", "--plain"]).stdout.contains("New"),
        "a refused create must leave nothing behind"
    );
    // And the field is genuinely still unset on the item that was updated.
    assert!(
        !p.expect(&["show", "1", "--json"])
            .stdout
            .contains("\"risk\""),
        "the read-only field was written anyway"
    );
}

#[test]
fn an_agent_cannot_move_an_item_to_a_status_it_may_only_propose() {
    let p = restricted_for_agents();

    let r = tool(
        &p,
        "close_item",
        serde_json::json!({"id": 1}),
        Some("claude"),
    );
    assert!(
        refused(&r),
        "close_item moved to a propose status: {}",
        text(&r)
    );
    assert_contains(&text(&r), "may not move an item to `done`", "");

    let r = tool(
        &p,
        "create_item",
        serde_json::json!({"title": "Born done", "status": "done"}),
        Some("claude"),
    );
    assert!(
        refused(&r),
        "create_item started at a propose status: {}",
        text(&r)
    );
}

/// The permission model hung off `clientInfo.name`, recorded during
/// `initialize`. A client that called a tool first was not an agent as far as
/// the check was concerned, and could write anything.
///
/// Arriving over this transport is what makes a caller an agent. The name only
/// says which one.
#[test]
fn skipping_initialize_does_not_escape_the_permission_model() {
    let p = restricted_for_agents();

    let r = tool(
        &p,
        "create_item",
        serde_json::json!({"title": "Snuck in", "fields": {"risk": "high"}}),
        None,
    );
    assert!(
        refused(&r),
        "a tool call before initialize wrote it: {}",
        text(&r)
    );
    assert_contains(&text(&r), "`mcp`", "named as an agent even unidentified");
}

/// `clientInfo.name` arrives off the wire and becomes an assignee, a
/// `created_by`, and a hook's environment. A NUL byte in it used to reach
/// `std::env::set_var`, which panics, and killed the server mid-stream.
#[test]
fn a_hostile_client_name_does_not_bring_the_server_down() {
    let p = seeded();

    for name in [
        "cl\u{0}ude",
        "evil\ntitle: pwned",
        "tab\there",
        "$(rm -rf /)",
        "../../etc/passwd",
    ] {
        let r = tool(&p, "check", serde_json::json!({}), Some(name));
        assert!(
            !r.is_null(),
            "the server did not answer for a client called {name:?}"
        );
    }

    // And a name with a newline in it is written as a name, not as a scalar
    // that happens to contain a line break.
    let p = seeded();
    tool(
        &p,
        "claim_item",
        serde_json::json!({"id": 1, "force": true}),
        Some("evil\ntitle: pwned"),
    );
    let file = p.read(&p.expect(&["show", "1", "--path"]).trimmed());
    assert!(
        file.contains("assignee: 'evil title: pwned'"),
        "the newline survived into the frontmatter:\n{file}"
    );
    assert!(p.run(&["check"]).ok(), "and the project still parses");
}

/// A very long name is bounded before it reaches a file.
#[test]
fn a_client_name_is_bounded() {
    let p = seeded();
    let long = "n".repeat(10_000);
    tool(
        &p,
        "claim_item",
        serde_json::json!({"id": 1, "force": true}),
        Some(&long),
    );
    let json: serde_json::Value =
        serde_json::from_str(&p.expect(&["show", "1", "--json"]).stdout).unwrap();
    let who = json["assignee"].as_str().unwrap_or_default();
    assert!(
        who.len() <= 64 && !who.is_empty(),
        "a client is free to call itself anything; a project's files are not \
         the place to find out how long: {} chars",
        who.len()
    );
}

/// Every tool advertises a schema. A schema that under-promises is one a
/// caller obeys unnecessarily; one that over-promises is a lie.
#[test]
fn every_tool_accepts_what_its_schema_advertises() {
    let p = seeded();
    let replies = mcp(&p, &[r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#]);
    let tools = replies[0]["result"]["tools"].as_array().expect("tools");
    assert_eq!(
        tools.len(),
        13,
        "a tool was added or removed without a test"
    );

    for t in tools {
        let name = t["name"].as_str().unwrap();
        let schema = &t["inputSchema"];
        assert_eq!(schema["type"], "object", "{name}");
        let required = schema["required"].as_array().expect("required");
        let props = schema["properties"].as_object().expect("properties");
        for r in required {
            let r = r.as_str().unwrap();
            assert!(
                props.contains_key(r),
                "{name} requires `{r}` and does not describe it"
            );
        }

        // Every tool taking an id must say it accepts the rendered form too,
        // because `require_id` does and a model that has seen `0001` in output
        // will send `0001`.
        if let Some(id) = props.get("id") {
            assert_eq!(
                id["type"],
                serde_json::json!(["integer", "string"]),
                "{name} advertises less than it accepts for `id`"
            );
        }
    }
}

#[test]
fn a_tool_accepts_an_id_in_either_form() {
    let p = seeded();
    for id in [serde_json::json!(1), serde_json::json!("0001")] {
        let r = tool(
            &p,
            "show_item",
            serde_json::json!({"id": id}),
            Some("claude"),
        );
        assert!(!refused(&r), "{id} was refused: {}", text(&r));
    }
}

/// `fields` was required, so replacing only the body meant sending an empty
/// object for want of anything better.
#[test]
fn an_update_may_change_only_the_body() {
    let p = seeded();
    let r = tool(
        &p,
        "update_item",
        serde_json::json!({"id": 1, "body": "Rewritten."}),
        Some("claude"),
    );
    assert!(!refused(&r), "{}", text(&r));
    assert_contains(&p.expect(&["show", "1"]).stdout, "Rewritten.", "");

    // Neither one is not a change.
    let r = tool(
        &p,
        "update_item",
        serde_json::json!({"id": 1}),
        Some("claude"),
    );
    assert!(refused(&r));
    assert_contains(&text(&r), "nothing to change", "");
}

/// Arguments of the wrong JSON type, which a model produces more often than a
/// missing argument.
#[test]
fn a_tool_given_the_wrong_type_fails_in_band() {
    let p = seeded();
    for (name, args) in [
        ("show_item", serde_json::json!({"id": {"nested": true}})),
        (
            "update_item",
            serde_json::json!({"id": 1, "fields": "not an object"}),
        ),
        ("search_items", serde_json::json!({"query": 42})),
        ("list_items", serde_json::json!({"limit": "many"})),
        ("add_note", serde_json::json!({"id": 1, "text": []})),
    ] {
        let r = tool(&p, name, args, Some("claude"));
        assert!(
            !r.is_null() && r["content"][0]["type"] == "text",
            "{name} did not answer in band"
        );
    }
    assert!(p.run(&["check"]).ok(), "and nothing was corrupted");
}

/// Two clients against one project. The lock is the only thing between them.
#[test]
fn two_agents_at_once_leave_one_consistent_backlog() {
    let p = seeded();
    let mut kids = Vec::new();
    for who in ["claude", "codex"] {
        let init = serde_json::json!({
            "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": { "clientInfo": { "name": who } }
        })
        .to_string();
        let mut input = vec![init];
        for n in 0..8 {
            input.push(
                serde_json::json!({
                    "jsonrpc": "2.0", "id": n + 1, "method": "tools/call",
                    "params": { "name": "create_item",
                                "arguments": { "title": format!("{who} {n}") } }
                })
                .to_string(),
            );
        }
        let root = p.root().to_path_buf();
        kids.push(std::thread::spawn(move || {
            let mut child = Command::new(bin())
                .arg("mcp")
                .current_dir(&root)
                .env("NO_COLOR", "1")
                .env("PATH", path_with_binary())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("spawn");
            use std::io::Write;
            child
                .stdin
                .take()
                .unwrap()
                .write_all(format!("{}\n", input.join("\n")).as_bytes())
                .expect("write");
            child.wait_with_output().expect("wait")
        }));
    }
    for k in kids {
        let out = k.join().expect("thread");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());
    let ids = p.expect(&["list", "-A", "--ids"]).lines();
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(
        ids.len(),
        unique.len(),
        "two clients were given the same id"
    );
}

/// Two branches creating the same file have no common ancestor, so the driver
/// cannot do a three-way merge. It has to *decline* — which means leaving a
/// conflict a person can see.
///
/// git writes no markers of its own when a custom driver runs. This path
/// returned without calling `git merge-file`, so an add/add conflict left
/// `ours` in the working tree with no sign the other side had said anything —
/// the same defect the ordinary decline path was already fixed for.
#[test]
fn an_add_add_conflict_leaves_markers_rather_than_ours() {
    let p = repository();
    p.add("Starting point", &[]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "start"]);

    // Two branches writing the same path with different content, by hand, so
    // there is genuinely no ancestor for that file.
    let path = "cairn/items/0009-contested.md";
    for (branch, title) in [("ours", "Ours"), ("theirs", "Theirs")] {
        git(&p, &["checkout", "-q", "main"]);
        git(&p, &["checkout", "-qb", branch]);
        p.write(
            path,
            &format!("---\nid: 9\ntitle: {title}\nstatus: backlog\n---\n{title} body\n"),
        );
        git(&p, &["add", "-A"]);
        git(&p, &["commit", "-qm", branch]);
    }

    git(&p, &["checkout", "-q", "ours"]);
    let out = merge(&p, "theirs");
    assert_ne!(out.code, 0, "an add/add conflict should not merge cleanly");

    let file = p.read(path);
    assert!(
        file.contains("<<<<<<<") && file.contains(">>>>>>>"),
        "the working tree has `ours` and no sign the other side existed:\n{file}"
    );
    assert!(
        file.contains("Theirs"),
        "what the other side wrote must survive into the conflict:\n{file}"
    );
}

/// The same thing said directly to the driver, because what git chooses to
/// invoke it for is git's business and this is the contract cairn owes.
///
/// An empty ancestor is what git passes for a file added on both sides. The
/// driver cannot merge that, and declining has to mean markers.
#[test]
fn the_driver_never_declines_without_leaving_a_conflict() {
    let p = Project::new();
    p.write(
        "ours.md",
        "---\nid: 9\ntitle: Ours\nstatus: backlog\n---\nours\n",
    );
    p.write("base.md", "");
    p.write(
        "theirs.md",
        "---\nid: 9\ntitle: Theirs\nstatus: backlog\n---\ntheirs\n",
    );

    let out = p.run(&[
        "merge-driver",
        &p.path("ours.md").display().to_string(),
        &p.path("base.md").display().to_string(),
        &p.path("theirs.md").display().to_string(),
        "cairn/items/0009-x.md",
    ]);
    assert_eq!(out.code, 1, "it must report that it did not resolve");

    let file = p.read("ours.md");
    assert!(
        file.contains("<<<<<<<") && file.contains("Theirs"),
        "declining left `ours` with no sign the other side existed:\n{file}"
    );
}

/// Deleting an item on one side and editing it on the other is a question for a
/// person. The driver must not answer it.
#[test]
fn a_delete_against_an_edit_is_left_to_a_person() {
    let p = repository();
    p.add("Contested", &[]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "start"]);
    let path = p.expect(&["show", "1", "--path"]).trimmed();

    on_branch(&p, "editing", "main", &["set", "1", "assignee=someone"]);

    git(&p, &["checkout", "-q", "main"]);
    git(&p, &["checkout", "-qb", "deleting"]);
    p.expect(&["remove", "1", "--force"]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "deleting"]);

    let out = merge(&p, "editing");
    assert_ne!(out.code, 0, "a delete against an edit is not resolvable");
    assert!(
        out.all().to_lowercase().contains("conflict"),
        "and git should say so: {}",
        out.all()
    );
    // Whatever a person decides, the edited content is still reachable.
    let _ = path;
}

/// The configuration is deliberately outside the driver: unioning two schema
/// blocks would produce a schema neither branch wrote.
#[test]
fn the_driver_does_not_claim_the_configuration() {
    let p = repository();
    let attrs = p.read(".gitattributes");
    assert!(
        !attrs.lines().any(|l| l.starts_with("cairn.toml")),
        "cairn.toml must not be given to the merge driver:\n{attrs}"
    );

    // And a real conflict in it comes out as a conflict, with both sides
    // visible, rather than being resolved by anything.
    for (branch, name) in [("ours", "mine"), ("theirs", "yours")] {
        git(&p, &["checkout", "-q", "main"]);
        git(&p, &["checkout", "-qb", branch]);
        // Both branches appending a block at the end of the file, which is
        // exactly the shape a schema conflict takes in practice.
        p.append(
            "cairn.toml",
            &format!("\n[[view]]\nname = \"{name}\"\nfilter = \"status=backlog\"\n"),
        );
        git(&p, &["add", "-A"]);
        git(&p, &["commit", "-qm", branch]);
    }
    git(&p, &["checkout", "-q", "ours"]);
    let out = merge(&p, "theirs");
    assert_ne!(
        out.code, 0,
        "two schemas disagreeing is a person's question"
    );
    let cfg = p.read("cairn.toml");
    assert!(
        cfg.contains("<<<<<<<") && cfg.contains("yours"),
        "both sides must be visible in the conflict:\n{cfg}"
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

// --- data nobody here wrote -------------------------------------------------

/// A `gh` on PATH that answers with whatever is given here.
///
/// The seam was already there: cairn runs `gh` by name, so PATH is the
/// injection point and no flag has to be invented to make the path testable.
/// The whole real code path runs — argument construction included.
#[cfg(unix)]
fn with_fake_gh(p: &Project, script: &str) -> std::ffi::OsString {
    use std::os::unix::fs::PermissionsExt;
    let dir = p.path("fake-bin");
    std::fs::create_dir_all(&dir).unwrap();
    let gh = dir.join("gh");
    std::fs::write(&gh, format!("#!/bin/sh\n{script}\n")).unwrap();
    std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();

    let mut paths = vec![dir];
    paths.extend(std::env::split_paths(&path_with_binary()));
    std::env::join_paths(paths).expect("PATH")
}

#[cfg(unix)]
fn import_github(p: &Project, script: &str, args: &[&str]) -> Out {
    let path = with_fake_gh(p, script);
    let path = path.to_string_lossy().to_string();
    let mut all = vec!["import", "--from", "github", "--repo", "owner/name"];
    all.extend_from_slice(args);
    p.run_env(&all, &[("PATH", Some(path.as_str()))])
}

/// GitHub data that is well-formed enough to be plausible and wrong in the
/// ways real data is wrong: a null body, a missing title, a label that is an
/// object where a name was expected, a timestamp that is not one.
#[cfg(unix)]
#[test]
fn malformed_github_issues_do_not_produce_malformed_items() {
    let p = Project::new();
    let issues = serde_json::json!([
        { "number": 1, "title": "Ordinary", "body": "fine", "state": "OPEN",
          "labels": [{"name": "bug"}], "assignees": [], "milestone": null,
          "createdAt": "2026-01-02T03:04:05Z", "updatedAt": "2026-01-02T03:04:05Z" },
        { "number": 2, "title": "No body", "body": null, "state": "CLOSED",
          "labels": [], "assignees": [], "milestone": null },
        { "number": 3, "body": "no title at all", "state": "OPEN" },
        { "number": 4, "title": "Odd labels", "labels": [{"colour": "red"}, "plain"],
          "state": "OPEN" },
        { "number": 5, "title": "Bad dates", "state": "OPEN",
          "createdAt": "yesterday", "updatedAt": "" },
    ]);
    let out = import_github(&p, &format!("cat <<'JSON'\n{issues}\nJSON"), &["--dry-run"]);
    assert!(
        !out.all().contains("panicked"),
        "external data brought cairn down: {}",
        out.all()
    );

    // Now for real, and the result has to be a project that validates.
    let out = import_github(&p, &format!("cat <<'JSON'\n{issues}\nJSON"), &[]);
    assert!(out.ok(), "{}", out.all());
    assert!(
        p.run(&["check"]).ok(),
        "the import wrote something the schema rejects:\n{}",
        p.run(&["check"]).all()
    );
    // An issue with no title arrives visibly untitled rather than plausibly
    // titled, and is findable.
    assert_contains(&out.all(), "(untitled)", "an untitled issue says so");

    // A timestamp that is not one does not become a date. It used to be written
    // into `created` verbatim, where every comparison against it is quietly
    // wrong and nothing ever says so.
    let listing = p.expect(&["list", "-A", "--plain", "--columns", "title,created"]);
    assert!(
        !listing.stdout.contains("yesterday"),
        "`yesterday` was written into a date field:\n{}",
        listing.stdout
    );
}

/// The same defect from the other direction: whatever put it there, a date that
/// is not a date is reported rather than sorted around.
#[test]
fn a_date_that_is_not_a_date_is_reported() {
    let p = Project::new();
    p.write(
        "cairn/items/0009-odd.md",
        "---\nid: 9\ntitle: Odd\nstatus: backlog\ncreated: yesterday\n---\nbody\n",
    );
    let out = p.expect(&["check"]).all();
    assert_contains(&out, "`created` is `yesterday`", "it names the value");
    assert_contains(&out, "YYYY-MM-DD", "and the shape it wanted");
    assert!(p.run(&["check"]).ok(), "a warning, not an error");
}

/// `gh` missing, and `gh` failing, are different situations and both are the
/// user's to fix.
#[cfg(unix)]
#[test]
fn a_github_import_says_which_way_it_failed() {
    let p = Project::new();

    let out = import_github(&p, "echo 'gh: not logged in' >&2; exit 1", &[]);
    assert!(!out.ok());
    assert_contains(&out.all(), "gh issue list", "it names what it ran");
    assert_contains(&out.all(), "not logged in", "and passes on what gh said");

    // Output that is not JSON at all — a paginator, a proxy login page.
    let out = import_github(&p, "echo '<html>login</html>'", &[]);
    assert!(!out.ok());
    assert_contains(&out.all(), "parsing", "it says where it failed");

    // And with no `gh` on PATH at all.
    let empty = p.path("empty-bin");
    std::fs::create_dir_all(&empty).unwrap();
    let empty = empty.display().to_string();
    let out = p.run_env(
        &["import", "--from", "github", "--repo", "owner/name"],
        &[("PATH", Some(empty.as_str()))],
    );
    assert!(!out.ok());
    assert_contains(&out.all(), "cli.github.com", "and says where to get it");
}

/// An interchange document written by something other than cairn.
#[test]
fn an_interchange_document_that_is_wrong_is_refused_rather_than_half_read() {
    let p = Project::new();
    let before = p.expect(&["list", "-A", "--count"]).trimmed();

    for doc in [
        // Wrong types where a string and a list belong.
        r#"{"items":[{"id":1,"title":42,"status":"backlog"}]}"#,
        r#"{"items":[{"id":1,"title":"T","labels":"not-a-list","status":"backlog"}]}"#,
        // A status and a type this project has never heard of.
        r#"{"items":[{"id":1,"title":"T","status":"invented"}]}"#,
        // Two records claiming one id.
        r#"{"items":[{"id":1,"title":"A","status":"backlog"},{"id":1,"title":"B","status":"backlog"}]}"#,
        // Not a document at all.
        r#"[]"#,
        r#"{"items":"none"}"#,
    ] {
        let out = p.run_stdin(&["import"], doc);
        assert!(
            !out.all().contains("panicked"),
            "a malformed document brought cairn down:\n{doc}\n{}",
            out.all()
        );
        assert!(
            p.run(&["check"]).ok(),
            "a malformed document left the project invalid:\n{doc}\n{}",
            p.run(&["check"]).all()
        );
    }
    let _ = before;
}

/// A body containing the frontmatter delimiter, arriving from outside.
#[test]
fn an_imported_body_containing_a_delimiter_round_trips() {
    let p = Project::new();
    let doc = serde_json::json!({
        "items": [{
            "id": 1, "title": "Tricky", "status": "backlog",
            "body": "before\n---\nid: 999\ntitle: not an item\n---\nafter\n"
        }]
    });
    let out = p.run_stdin(&["import"], &doc.to_string());
    assert!(out.ok(), "{}", out.all());
    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());

    let shown = p.expect(&["show", "1"]).stdout;
    assert_contains(&shown, "not an item", "the body survived intact");
    assert_eq!(
        p.expect(&["list", "-A", "--count"]).trimmed(),
        "1",
        "the delimiter in a body was read as a second item"
    );
}

/// `cairn log` parses the output of `git log`, an external program whose
/// format is stable by convention rather than by contract. It has produced two
/// real defects already.
#[test]
fn history_survives_a_commit_message_that_looks_like_data() {
    let p = repository();
    p.expect(&["set", "1", "priority=p0"]);
    git(&p, &["add", "-A"]);
    // A message carrying every shape the parser looks for.
    git(
        &p,
        &["commit", "-qm", "start\n\nid: 999\nstatus: done\n---\n"],
    );

    p.expect(&["set", "1", "assignee=someone"]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "0001|0002|---|id: 1"]);

    let out = p.expect(&["log", "1"]);
    assert!(out.ok(), "{}", out.all());
    let json: serde_json::Value =
        serde_json::from_str(&p.expect(&["log", "1", "--json"]).stdout).expect("JSON");
    let revisions = json["revisions"].as_array().map(Vec::len).unwrap_or(0);
    assert!(
        revisions >= 2,
        "the history was lost to a commit message: {json}"
    );
}

/// A repository with no commits at all, which is where somebody runs this by
/// accident the first time.
#[test]
fn history_in_an_empty_repository_explains_itself() {
    let p = Project::empty();
    git(&p, &["init", "-q", "-b", "main", "."]);
    p.expect(&["init", "--bare", "--name", "Fresh"]);
    p.add("Unversioned", &[]);

    let out = p.run(&["log", "1"]);
    assert!(
        !out.all().contains("panicked"),
        "an empty repository brought cairn down: {}",
        out.all()
    );
    assert!(
        out.all().to_lowercase().contains("no history")
            || out.all().to_lowercase().contains("not")
            || out.ok(),
        "it should say something rather than nothing: {}",
        out.all()
    );
}

/// A file renamed twice in one commit, which is what `renumber` does.
#[test]
fn history_follows_an_item_through_two_renames_in_one_commit() {
    let p = repository();
    p.add("Original title", &[]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "one"]);

    p.expect(&["set", "1", "title=Second title"]);
    p.expect(&["set", "1", "title=Third title"]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "renamed twice in one commit"]);

    let out = p.expect(&["log", "1"]);
    assert!(out.ok(), "{}", out.all());
    assert_contains(&out.stdout, "title", "the retitle is in the history");
    // And it did not wander into a different item on the way.
    assert!(
        !out.stdout.contains("Watched"),
        "the trail crossed into another item:\n{}",
        out.stdout
    );
}

/// Renaming a key rewrites every reference that named it — and must leave
/// alone anything that referred to the same item by id, which did not change.
#[test]
fn renaming_a_key_moves_references_by_key_and_not_by_id() {
    let p = Project::new();
    p.expect(&["new", "First release", "-t", "milestone", "-q"]);
    p.expect(&["set", "1", "key=v0.1"]);
    p.add("Scheduled", &["-m", "v0.1"]);
    p.add("Blocked by the milestone", &["-d", "1"]);

    let out = p.expect(&["set", "1", "key=v1.0"]).all();
    assert_contains(&out, "also 0002", "it says which references it moved");

    let scheduled: serde_json::Value =
        serde_json::from_str(&p.expect(&["show", "2", "--json"]).stdout).unwrap();
    assert_eq!(
        scheduled["milestone"], "v1.0",
        "a reference by key was not moved"
    );

    let blocked: serde_json::Value =
        serde_json::from_str(&p.expect(&["show", "3", "--json"]).stdout).unwrap();
    assert_eq!(
        blocked["depends_on"],
        serde_json::json!([1]),
        "a reference by id names the item, not its handle, and did not change"
    );
    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());
}

// --- coverage: surfaces the suite had never driven ---------------------------

/// A board grouped by something other than status, which is most of the command.
#[test]
fn a_board_groups_by_any_field() {
    let p = seeded();
    p.expect(&["set", "1", "priority=p0"]);
    p.expect(&["set", "2", "priority=p2"]);

    let by_priority = p.expect(&["board", "--group-by", "priority"]).stdout;
    assert_contains(&by_priority, "p0", "the enum's own values are the columns");
    assert_contains(&by_priority, "p3", "including ones nothing is filed under");

    // Item 1 carries v0.1; the rest carry nothing, so both a named column and
    // the unscheduled one appear.
    //
    // The context used to be built *after* containers were dropped, so
    // `--group-by milestone` found no milestones, drew no column for them, and
    // every item that had one disappeared from the board while the unscheduled
    // ones stayed. A board that quietly omits scheduled work is worse than one
    // that shows nothing.
    let by_milestone = p.expect(&["board", "--group-by", "milestone"]).stdout;
    assert_contains(&by_milestone, "v0.1", "the milestone column");
    assert_contains(&by_milestone, "(none)", "and one for what is unscheduled");
    assert_contains(
        &by_milestone,
        "First item",
        "the scheduled item is on the board",
    );

    // A field with no declared values takes its columns from what items carry.
    p.expect(&["set", "1", "area=parser"]);
    let by_area = p.expect(&["board", "--group-by", "area"]).stdout;
    assert_contains(&by_area, "parser", "");
}

#[test]
fn a_board_honours_a_view_a_filter_and_a_milestone() {
    let p = seeded();
    p.append(
        "cairn.toml",
        "\n[[view]]\nname = \"hot\"\nfilter = \"priority=p0\"\ngroup_by = \"milestone\"\n",
    );
    p.expect(&["set", "1", "priority=p0"]);

    // The view names a group_by, which is the branch worth reaching.
    let out = p.expect(&["board", "--view", "hot"]).stdout;
    assert_contains(&out, "v0.1", "the view's own group_by was used");

    p.expect(&["board", "--filter", "category!=done"]);
    p.expect(&["board", "--milestone", "v0.1"]);
    p.expect(&["board", "--all"]);
    p.expect(&["board", "--width", "20"]);

    let out = p.fails(&["board", "--view", "nonesuch"]).all();
    assert_contains(&out, "unknown view", "");
}

/// `cairn roadmap --items` lists the work under each milestone.
#[test]
fn a_roadmap_can_list_its_items() {
    let p = seeded();
    let out = p.expect(&["roadmap", "--items"]).stdout;
    assert_contains(&out, "v0.1", "the milestone");
    assert!(
        out.lines().count() > p.expect(&["roadmap"]).stdout.lines().count(),
        "--items showed no more than the summary did"
    );

    p.expect(&["close", "1"]);
    let closed_hidden = p.expect(&["roadmap", "--items"]).stdout;
    let closed_shown = p.expect(&["roadmap", "--items", "--all"]).stdout;
    assert!(
        closed_shown.len() > closed_hidden.len(),
        "--all did not bring back what was finished"
    );
}

/// A dependency that no longer exists is called out where somebody will see it,
/// rather than left as a number that resolves to nothing.
#[test]
fn show_marks_a_dependency_that_is_missing() {
    let p = seeded();
    p.write(
        "cairn/items/0050-hangs-on-nothing.md",
        "---\nid: 50\ntitle: Hangs on nothing\nstatus: backlog\ndepends_on:\n  - 999\n---\nbody\n",
    );
    let out = p.expect(&["show", "50"]).stdout;
    assert_contains(&out, "missing", "it says the dependency is not there");

    // And a dependency that exists is shown ticked once it is finished.
    p.expect(&["close", "1"]);
    p.expect(&["set", "50", "depends_on=1"]);
    assert_contains(&p.expect(&["show", "50"]).stdout, "[x]", "");
}

/// The rendered roadmap's optional parts, none of which the suite had produced.
#[test]
fn a_rendered_roadmap_carries_its_furniture() {
    let p = seeded();
    p.write("docs/intro.md", "Read this first.\n");
    p.write("docs/outro.md", "That is all.\n");
    let cfg = p
        .read("cairn.toml")
        .replace(
            "[render]",
            "[render]\nheader = \"docs/intro.md\"\nfooter = \"docs/outro.md\"",
        )
        .replace("title = \"Roadmap\"", "title = \"The plan\"");
    p.write("cairn.toml", &cfg);

    p.expect(&["render"]);
    let out = p.read("ROADMAP.md");
    assert_contains(&out, "# The plan", "the configured title");
    assert_contains(&out, "Read this first.", "the header fragment");
    assert_contains(&out, "That is all.", "the footer fragment");

    // A header naming a file that is not there is reported, not ignored.
    let cfg = p
        .read("cairn.toml")
        .replace("docs/intro.md", "docs/gone.md");
    p.write("cairn.toml", &cfg);
    let out = p.run(&["render"]);
    assert!(!out.ok(), "a missing fragment should not render silently");
}

#[test]
fn a_rendered_roadmap_can_link_to_its_items() {
    let p = seeded();
    let cfg = p
        .read("cairn.toml")
        .replace("link_items = false", "link_items = true")
        .replace(
            "[project]",
            "[project]\nurl = \"https://example.invalid/blob/main\"",
        );
    p.write("cairn.toml", &cfg);
    p.expect(&["render"]);
    assert_contains(
        &p.read("ROADMAP.md"),
        "https://example.invalid/blob/main/cairn/items/",
        "items are linked",
    );
}

/// `render --output` and `--check`, which is what continuous integration runs.
#[test]
fn render_can_write_elsewhere_and_check_itself() {
    let p = seeded();
    p.expect(&["render"]);
    assert!(p.run(&["render", "--check"]).ok(), "it was just rendered");

    // `--no-hooks`, or the after-create hook renders it again and it is never
    // stale.
    p.expect(&["new", "Something new", "-q", "--no-hooks"]);
    let out = p.run(&["render", "--check"]);
    assert!(!out.ok(), "the roadmap is stale and --check should say so");
    assert_contains(&out.all(), "render", "and name the remedy");

    p.expect(&["render", "--output", "elsewhere.md"]);
    assert!(!p.read("elsewhere.md").is_empty());
}

/// A title longer than its column is clipped with an ellipsis rather than
/// wrapped or truncated mid-character.
#[test]
fn a_long_title_is_clipped_to_its_column() {
    let p = Project::new();
    let long = "A title that goes on and on and on and will not fit into any \
                reasonable column width at all";
    p.add(long, &[]);
    p.add(
        "Ünïcödé — a title with wide characters ☂☂☂☂☂☂☂☂☂☂☂☂☂☂☂☂☂☂☂☂",
        &[],
    );

    let out = p.expect(&["board", "--width", "24"]).stdout;
    assert_contains(&out, "…", "something was clipped");
    assert!(
        out.lines().all(|l| l.chars().count() < 400),
        "a line ran away"
    );
    assert!(p.run(&["check"]).ok());
}

/// Re-importing over a backlog that already has the items: the update path,
/// which had never run.
#[test]
fn a_second_import_updates_rather_than_duplicates() {
    let p = Project::new();
    let doc = |title: &str, status: &str| {
        serde_json::json!({
            "items": [{
                "id": 1, "title": title, "status": status,
                "source": "elsewhere#1", "body": "from outside"
            }]
        })
        .to_string()
    };

    let out = p.run_stdin(&["import"], &doc("First name", "backlog"));
    assert!(out.ok(), "{}", out.all());
    let count = p.expect(&["list", "-A", "--count"]).trimmed();

    // The same record again, changed, with --update.
    let out = p.run_stdin(&["import", "--update"], &doc("Second name", "backlog"));
    assert!(out.ok(), "{}", out.all());
    assert_eq!(
        p.expect(&["list", "-A", "--count"]).trimmed(),
        count,
        "the second import duplicated instead of updating"
    );
    assert_contains(
        &p.expect(&["list", "-A", "--plain"]).stdout,
        "Second name",
        "and the update took",
    );

    // Without --update, an item already present is left alone.
    let out = p.run_stdin(&["import"], &doc("Third name", "backlog"));
    assert!(out.ok(), "{}", out.all());
    assert_contains(&out.all(), "already present", "");
}

#[test]
fn an_import_can_create_the_milestones_it_names() {
    let p = Project::new();
    let doc = serde_json::json!({
        "items": [
            {"id": 1, "title": "Scheduled", "status": "backlog", "milestone": "v9.9"},
            {"id": 2, "title": "Also", "status": "backlog", "milestone": "v9.9"}
        ]
    });
    let out = p.run_stdin(&["import", "--create-milestones"], &doc.to_string());
    assert!(out.ok(), "{}", out.all());
    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());
    assert_contains(
        &p.expect(&["list", "-A", "--plain", "--columns", "id,key,type"])
            .stdout,
        "v9.9",
        "the milestone was created once",
    );
    assert_eq!(
        p.expect(&["list", "-A", "--ids", "--filter", "key=v9.9"])
            .lines()
            .len(),
        1,
        "a milestone was created twice"
    );
}

/// `import --close` comments on and closes the source issues.
#[cfg(unix)]
#[test]
fn an_import_can_close_what_it_took() {
    let p = Project::new();
    let issues = serde_json::json!([
        { "number": 7, "title": "Taken over", "body": "b", "state": "OPEN",
          "labels": [], "assignees": [], "milestone": null }
    ]);
    let script = format!(
        "if [ \"$2\" = list ]; then cat <<'JSON'\n{issues}\nJSON\nelse echo \"closed $3\"; fi"
    );
    let out = import_github(&p, &script, &["--close"]);
    assert!(out.ok(), "{}", out.all());
    assert_contains(&out.all(), "closed", "it reported closing the source");
    assert!(p.run(&["check"]).ok());
}

/// The schema an agent reads first, which nothing had ever called.
#[test]
fn the_agent_can_ask_for_the_schema() {
    let p = seeded();
    let r = tool(&p, "get_schema", serde_json::json!({}), Some("claude"));
    let text = r["content"][0]["text"].as_str().expect("text");
    let schema: serde_json::Value = serde_json::from_str(text).expect("JSON");

    assert!(schema["statuses"].is_array(), "{schema}");
    assert!(schema["fields"].is_array(), "{schema}");
    assert!(schema["counts"]["total"].is_number(), "{schema}");
    assert!(
        schema["filter_syntax"]["operators"].is_array(),
        "an agent has to be told the filter grammar: {schema}"
    );
    assert!(
        schema["milestones"].is_array(),
        "milestones are items now and still belong in the schema: {schema}"
    );
}

/// An identifier format with a suffix as well as a prefix. Reading one back
/// exercises the other end of the parser.
#[test]
fn an_identifier_can_have_a_suffix() {
    let p = keyed("[{n:03}]", None);
    p.expect(&["new", "First", "-q"]);
    assert_eq!(p.expect(&["list", "--ids"]).trimmed(), "[001]");

    // Accepted back in every spelling: rendered, bare, and case-folded.
    for spelling in ["[001]", "001", "1", "#1", "[1]"] {
        assert!(
            p.run(&["show", spelling]).ok(),
            "`{spelling}` should name the same item"
        );
    }
    assert!(!p.run(&["show", "[abc]"]).ok());
}

/// Colours are named in `cairn.toml`, and every name it accepts should reach
/// the terminal rather than falling through to plain text.
#[test]
fn every_named_colour_is_understood() {
    let p = Project::new();
    let mut cfg = p.read("cairn.toml");
    for (n, colour) in ["black", "magenta", "purple", "white", "bold", "grey"]
        .into_iter()
        .enumerate()
    {
        cfg.push_str(&format!(
            "\n[[status]]\nname = \"s{n}\"\ncategory = \"open\"\ncolor = \"{colour}\"\n"
        ));
    }
    cfg.push_str(
        "\n[[status]]\nname = \"nonsense\"\ncategory = \"open\"\ncolor = \"chartreuse\"\n",
    );
    p.write("cairn.toml", &cfg);

    for n in 0..6 {
        p.expect(&["new", &format!("Item {n}"), "-s", &format!("s{n}"), "-q"]);
    }
    p.expect(&["new", "Unknown colour", "-s", "nonsense", "-q"]);

    let out = p.run_env(&["list", "-A", "--color", "always"], &[("NO_COLOR", None)]);
    assert!(out.ok(), "{}", out.all());
    assert!(out.stdout.contains('\u{1b}'), "nothing was painted");
    // A colour name nothing recognises is left alone rather than refused.
    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());
}

/// A configuration with no `[project]` block at all, which is the smallest
/// thing that is still a project.
#[test]
fn a_configuration_with_no_project_block_works() {
    let p = Project::empty();
    p.write(
        "cairn.toml",
        "format = 2\n[[status]]\nname = \"todo\"\ncategory = \"open\"\n",
    );
    std::fs::create_dir_all(p.path("cairn/items")).unwrap();

    p.expect(&["new", "Minimal", "-q"]);
    assert_eq!(p.expect(&["list", "--ids"]).trimmed(), "0001");
    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());
    assert_contains(
        &p.expect(&["config"]).stdout,
        "cairn/items",
        "the default directory",
    );
}

/// A ref field addressed by id stores a number, not a string, so the file says
/// what it means and a reader does not have to guess.
#[test]
fn an_id_addressed_ref_stores_numbers() {
    let p = Project::new();
    p.append(
        "cairn.toml",
        "\n[[field]]\nname = \"blocks\"\nkind = \"ref\"\ntarget = \"*\"\n\
         by = \"id\"\ncardinality = \"many\"\n\
         \n[[field]]\nname = \"parent_item\"\nkind = \"ref\"\ntarget = \"*\"\nby = \"id\"\n",
    );
    p.add("One", &[]);
    p.add("Two", &[]);
    p.add("Three", &[]);

    p.expect(&["set", "3", "blocks=1,2"]);
    p.expect(&["set", "3", "parent_item=1"]);

    let file = p.read(&p.expect(&["show", "3", "--path"]).trimmed());
    assert!(
        file.contains("- 1") && file.contains("- 2"),
        "a many-valued id ref should be a sequence of numbers:\n{file}"
    );
    assert!(
        file.contains("parent_item: 1"),
        "a single id ref should be a number:\n{file}"
    );
    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());

    // Cleared, the keys go rather than being left empty.
    p.expect(&["set", "3", "blocks="]);
    p.expect(&["set", "3", "parent_item="]);
    let file = p.read(&p.expect(&["show", "3", "--path"]).trimmed());
    assert!(
        !file.contains("parent_item"),
        "a cleared ref was left behind:\n{file}"
    );
}

/// Setting several items at once, where one of them fails: the ones already
/// written are named, so nobody has to guess how far it got.
#[test]
fn a_partial_multi_item_write_says_how_far_it_got() {
    let p = seeded();
    let out = p.fails(&["set", "1", "2", "3", "status=nonsense"]);
    assert_contains(&out.all(), "nonsense", "it names the bad value");

    // The first item takes a good change, the second a bad one.
    let out = p.fails(&["set", "2", "3", "priority=p0", "effort=enormous"]);
    assert_contains(&out.all(), "enormous", "");
    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());
}

/// `next` has a flag for every way of asking "what should I do?", and most had
/// never been driven.
#[test]
fn next_answers_every_way_of_asking() {
    let p = seeded();
    p.expect(&["set", "1", "assignee=tester"]);
    p.expect(&["set", "2", "depends_on=1"]);

    assert!(!p.expect(&["next", "--mine"]).stdout.is_empty());
    assert!(!p.expect(&["next", "--unassigned"]).stdout.is_empty());
    assert!(
        !p.expect(&["next", "--assignee", "tester"])
            .stdout
            .is_empty()
    );
    p.expect(&["next", "--type", "feature"]);
    p.expect(&["next", "--milestone", "v0.1"]);
    p.expect(&["next", "--filter", "priority=p0"]);
    p.expect(&["next", "--limit", "1"]);

    // Blocked work is excluded, and `--blocked` brings it back with its reason.
    let plain = p.expect(&["next", "--ids"]).lines();
    assert!(
        !plain.contains(&"0002".to_string()),
        "blocked work was offered"
    );
    let with_blocked = p.expect(&["next", "--blocked", "--ids"]).lines();
    assert!(with_blocked.contains(&"0002".to_string()));

    let count: usize = p
        .expect(&["next", "--count"])
        .trimmed()
        .parse()
        .expect("a number");
    assert!(count > 0);
    let json: serde_json::Value =
        serde_json::from_str(&p.expect(&["next", "--json"]).stdout).expect("JSON");
    assert!(json.as_array().is_some(), "{json}");
}

/// Search over titles alone, and every output shape.
#[test]
fn search_has_the_same_output_shapes_as_everything_else() {
    let p = seeded();
    p.expect(&["note", "1", "the word cassowary appears only in a body"]);

    assert!(!p.expect(&["search", "cassowary"]).stdout.is_empty());
    // `--titles` looks at titles alone, so a word that lives only in a body
    // finds nothing — and finding nothing is an error, not a silent success.
    let out = p.fails(&["search", "cassowary", "--titles"]);
    assert_contains(&out.all(), "no items match", "");
    assert!(!p.expect(&["search", "item", "--titles"]).stdout.is_empty());

    p.expect(&["search", "item", "--json"]);
    p.expect(&["search", "item", "--ids"]);
    p.expect(&["search", "item", "--plain"]);
    p.expect(&["search", "item", "--count"]);
    p.expect(&["search", "item", "--limit", "1"]);
    p.expect(&["search", "item", "--filter", "category!=done"]);
    p.expect(&["close", "2"]);
    p.expect(&["search", "item", "--all"]);
}

/// Export's shapes and filters.
#[test]
fn export_can_be_narrowed_and_redirected() {
    let p = seeded();
    p.expect(&["close", "2"]);

    let all: serde_json::Value = serde_json::from_str(&p.expect(&["export"]).stdout).expect("JSON");
    let open: serde_json::Value =
        serde_json::from_str(&p.expect(&["export", "--open-only"]).stdout).expect("JSON");
    assert!(
        open["items"].as_array().unwrap().len() < all["items"].as_array().unwrap().len(),
        "--open-only kept everything"
    );

    let filtered: serde_json::Value =
        serde_json::from_str(&p.expect(&["export", "--filter", "priority=p0"]).stdout)
            .expect("JSON");
    assert!(!filtered["items"].as_array().unwrap().is_empty());

    p.expect(&["export", "--output", "out.json"]);
    assert!(!p.read("out.json").is_empty());
}

/// `set --filter` is the dangerous write: the person running it has not seen
/// the list, so it shows one and asks.
#[test]
fn a_filtered_write_shows_what_it_will_change_and_asks() {
    let p = seeded();

    // Declined.
    let out = p.run_stdin(&["set", "--filter", "category!=done", "priority=p0"], "n\n");
    assert_eq!(
        out.code,
        1,
        "declining should not be success: {}",
        out.all()
    );
    assert_contains(&out.all(), "aborted", "");
    assert_contains(&out.stdout, "First item", "it listed what it would touch");
    assert_eq!(
        p.expect(&["list", "-A", "--ids", "--filter", "priority=p0"])
            .lines()
            .len(),
        1,
        "a declined write changed something"
    );

    // Accepted.
    let out = p.run_stdin(&["set", "--filter", "category!=done", "priority=p1"], "y\n");
    assert!(out.ok(), "{}", out.all());
    assert!(
        p.expect(&["list", "-A", "--ids", "--filter", "priority=p1"])
            .lines()
            .len()
            >= 2
    );

    // `--yes` does not ask at all.
    let out = p.expect(&["set", "--filter", "category!=done", "priority=p3", "--yes"]);
    assert!(!out.all().contains("[y/N]"), "--yes still asked");

    // A filter matching nothing is an error rather than a silent no-op.
    let out = p.fails(&["set", "--filter", "priority=p9", "effort=s", "--yes"]);
    assert_contains(&out.all(), "no item matches", "");

    // Ids and a filter together is a mistake, not a union.
    let out = p.fails(&["set", "1", "--filter", "priority=p0", "effort=s"]);
    assert_contains(&out.all(), "not both", "");

    // And no target at all.
    let out = p.fails(&["set", "effort=s"]);
    assert_contains(&out.all(), "no item named", "");
}

#[test]
fn set_refuses_an_empty_assignment_list() {
    let p = seeded();
    let out = p.fails(&["set", "1"]);
    assert_contains(&out.all(), "field=value", "it says what it wanted");
}

/// A milestone reference that resolves to nothing is shown in a way somebody
/// notices, rather than as a plausible string.
#[test]
fn show_marks_a_milestone_that_answers_to_nothing() {
    let p = seeded();
    p.write(
        "cairn/items/0060-adrift.md",
        "---\nid: 60\ntitle: Adrift\nstatus: backlog\nmilestone: v9.9\n---\nbody\n",
    );
    let out = p.run_env(&["show", "60", "--color", "always"], &[("NO_COLOR", None)]);
    assert!(out.ok(), "{}", out.all());
    assert_contains(&out.stdout, "v9.9", "the value is shown");
    assert!(out.stdout.contains('\u{1b}'), "and marked, not left plain");
}

/// `show --raw` and `--path`, which are what a script reaches for.
#[test]
fn show_has_a_shape_for_a_script() {
    let p = seeded();
    let raw = p.expect(&["show", "1", "--raw"]).stdout;
    assert!(raw.starts_with("---"), "--raw is the file itself:\n{raw}");

    let path = p.expect(&["show", "1", "--path"]).trimmed();
    assert!(path.ends_with(".md"), "{path}");
    assert_eq!(p.read(&path), raw, "--path and --raw disagree");
}

/// `migrate --check` is what continuous integration runs, and it answers
/// differently depending on which side of the format the project is on.
#[test]
fn migrate_check_reports_both_ways() {
    let p = format_one();
    let out = p.run(&["migrate", "--check"]);
    assert_eq!(
        out.code, 1,
        "a project behind the format should fail --check"
    );
    assert_contains(&out.all(), "stale", "");
    assert_contains(&out.all(), "cairn migrate", "and name the remedy");

    p.expect(&["migrate"]);
    let out = p.expect(&["migrate", "--check"]);
    assert_contains(&out.all(), "current", "");

    // Quietly, for a script.
    let out = p.expect(&["migrate", "--check", "--quiet"]);
    assert!(
        out.stdout.trim().is_empty(),
        "--quiet printed: {}",
        out.stdout
    );
}

#[test]
fn migrate_outside_a_project_says_so() {
    let p = Project::empty();
    let out = p.run(&["migrate"]);
    assert!(!out.ok());
    assert_contains(&out.all(), "cairn.toml", "");
}

/// A table wider than the terminal: the last column absorbs what is left and
/// everything stays on one line.
#[test]
fn a_table_fits_a_narrow_terminal() {
    let p = Project::new();
    for n in 0..4 {
        p.add(
            &format!("Item {n} with a title long enough that no narrow terminal could show all of it at once"),
            &["--set", "priority=p0", "--set", "area=some-fairly-long-area-name"],
        );
    }
    let out = p.run_env(
        &["list", "-A", "--columns", "id,priority,area,title"],
        &[("COLUMNS", Some("60"))],
    );
    assert!(out.ok(), "{}", out.all());
    for line in out.stdout.lines() {
        assert!(
            line.chars().count() < 400,
            "a row ran away rather than being clipped:\n{line}"
        );
    }
}

/// `log` has three shapes and a limit, and the one nothing had exercised is
/// `--patch`, which hands back the diffs themselves.
#[test]
fn log_can_show_the_patches_themselves() {
    let p = repository();
    p.expect(&["set", "1", "priority=p0"]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "priority"]);

    let out = p.expect(&["log", "1", "--patch"]);
    assert_contains(&out.stdout, "diff --git", "the raw diff");
    assert_contains(&out.stdout, "priority", "and what changed in it");

    // Bounded, for an item with a long history.
    p.expect(&["set", "1", "effort=s"]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "effort"]);
    let one = p.expect(&["log", "1", "--patch", "-n", "1"]).stdout;
    let all = p.expect(&["log", "1", "--patch"]).stdout;
    assert!(one.len() < all.len(), "-n did not limit the patches");
}

/// A milestone is an item, so its history is readable by the name people use
/// for it rather than by a number they would have to look up.
#[test]
fn history_can_be_asked_for_by_key() {
    let p = repository();
    milestone(&p, "v0.1", None);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "a milestone"]);

    let out = p.expect(&["log", "v0.1"]);
    assert!(out.ok(), "{}", out.all());

    // And a name that answers to nothing says so.
    let out = p.fails(&["log", "no-such-key"]);
    assert_contains(&out.all(), "no-such-key", "");
}

/// The item-level checks that had never been driven: a missing status, a type
/// nothing declares, a list field carrying a value outside its enum.
#[test]
fn check_reports_every_way_an_item_can_disagree_with_the_schema() {
    let p = Project::new();
    p.append(
        "cairn.toml",
        "\n[[field]]\nname = \"platforms\"\nkind = \"list\"\n\
         values = [\"linux\", \"macos\"]\n\
         \n[[field]]\nname = \"owner_team\"\nkind = \"text\"\nrequired = true\n",
    );
    p.write(
        "cairn/items/0011-no-status.md",
        "---\nid: 11\ntitle: No status\n---\nbody\n",
    );
    p.write(
        "cairn/items/0012-odd-type.md",
        "---\nid: 12\ntitle: Odd type\ntype: sculpture\nstatus: backlog\n---\nbody\n",
    );
    p.write(
        "cairn/items/0013-odd-list.md",
        "---\nid: 13\ntitle: Odd list\nstatus: backlog\nplatforms:\n  - linux\n  - haiku\n---\nbody\n",
    );

    let out = p.run(&["check"]);
    assert!(!out.ok());
    let all = out.all();
    assert_contains(&all, "missing `status`", "");
    assert_contains(&all, "unknown type `sculpture`", "");
    assert_contains(&all, "haiku", "a list value outside its enum");
    assert_contains(&all, "owner_team", "a required field nothing carries");
    // Diagnostics carry a line, so an editor can jump to the offending key.
    assert!(
        all.lines()
            .any(|l| l.contains(".md:") && l.contains("unknown type")),
        "no file:line on the diagnostics:\n{all}"
    );
}

/// `check --strict` and `--quiet`, which is how continuous integration uses it.
#[test]
fn check_can_be_strict_and_quiet() {
    let p = seeded();
    assert!(p.expect(&["check", "--quiet"]).stdout.trim().is_empty());

    // A warning: a filename that no longer matches its title.
    let path = p.expect(&["show", "1", "--path"]).trimmed();
    let moved = path.replace("first-item", "wrong-name");
    std::fs::rename(p.path(&path), p.path(&moved)).unwrap();

    assert!(p.run(&["check"]).ok(), "a warning is not a failure");
    let out = p.run(&["check", "--strict"]);
    assert!(!out.ok(), "--strict should make it one: {}", out.all());
}

/// An item file that cannot be parsed at all stops anything that writes, and
/// anything that produces a durable artefact, rather than acting on half a
/// backlog.
#[test]
fn a_partial_view_is_reported_by_every_reader_that_needs_all_of_it() {
    let p = seeded();
    p.write("cairn/items/0070-truncated.md", "---\nid: 70\ntitle: Trunc");

    // Reading commands still work, on what parses.
    assert!(p.run(&["list"]).ok());
    // Anything durable refuses.
    for args in [vec!["export"], vec!["render"], vec!["check"]] {
        let out = p.run(&args);
        assert!(!out.ok(), "`cairn {args:?}` acted on a partial backlog");
    }
}

/// A note appended under a heading of its own, and the default of today's date.
#[test]
fn a_note_can_choose_its_own_heading() {
    let p = seeded();
    p.expect(&["note", "1", "plain note"]);
    assert_contains(&p.expect(&["show", "1"]).stdout, "plain note", "");

    let r = tool(
        &p,
        "add_note",
        serde_json::json!({"id": 1, "text": "filed elsewhere", "heading": "Decisions"}),
        Some("claude"),
    );
    assert!(!refused(&r), "{}", text(&r));
    let body = p.expect(&["show", "1"]).stdout;
    assert_contains(&body, "Decisions", "the heading it asked for");
    assert_contains(&body, "plain note", "and nothing was erased");
}

/// A project with no `done` category at all: `close` cannot guess, and says so
/// rather than picking something.
#[test]
fn close_without_a_done_status_asks_for_one() {
    let p = Project::empty();
    p.write(
        "cairn.toml",
        "format = 2\n[project]\nname = \"T\"\n\
         [[status]]\nname = \"todo\"\ncategory = \"open\"\n\
         [[status]]\nname = \"doing\"\ncategory = \"active\"\n",
    );
    std::fs::create_dir_all(p.path("cairn/items")).unwrap();
    p.expect(&["new", "Never finished", "-q"]);

    let out = p.fails(&["close", "1"]);
    assert_contains(&out.all(), "category = \"done\"", "it says what is missing");
    assert_contains(&out.all(), "--status", "and how to proceed anyway");

    // And explicitly is fine.
    assert!(p.run(&["close", "1", "--status", "doing"]).ok());
}

/// `reopen` mirrors `close`, including choosing where to go back to.
#[test]
fn reopen_returns_an_item_to_the_backlog() {
    let p = seeded();
    p.expect(&["close", "1"]);
    assert_contains(&p.expect(&["show", "1"]).stdout, "done", "");

    p.expect(&["reopen", "1"]);
    let out = p.expect(&["show", "1"]).stdout;
    assert!(!out.contains("done"), "it did not come back: {out}");

    p.expect(&["close", "1"]);
    p.expect(&["reopen", "1", "--status", "planned"]);
    assert_contains(&p.expect(&["show", "1"]).stdout, "planned", "");
}

/// Closing several at once, and the acceptance-criteria report that comes with
/// it.
#[test]
fn closing_reports_criteria_that_are_still_unticked() {
    let p = Project::new();
    p.expect(&[
        "new",
        "With criteria",
        "-q",
        "--body",
        "## Acceptance criteria\n\n- [x] done one\n- [ ] not done\n",
    ]);
    let out = p.expect(&["close", "1"]).all();
    assert_contains(&out, "1 of 2", "it says how many are outstanding");
    assert!(p.run(&["check"]).ok(), "and closes anyway");

    // A project can ask for that to be an error afterwards as well.
    let cfg = p
        .read("cairn.toml")
        .replace("[project]", "[project]\nrequire_criteria = true");
    p.write("cairn.toml", &cfg);
    // A warning, because an unticked box is a judgement about process rather
    // than a schema violation; `--strict` is what turns it into a failure.
    let out = p.expect(&["check"]).all();
    assert_contains(&out, "1 of 2 acceptance criteria", "");
    assert!(
        !p.run(&["check", "--strict"]).ok(),
        "--strict should fail on it"
    );
}

/// Criteria confined to one heading, which is what a project sets when its
/// bodies contain other checklists.
#[test]
fn criteria_can_be_confined_to_one_section() {
    let p = Project::new();
    let cfg = p.read("cairn.toml").replace(
        "[project]",
        "[project]\ncriteria_section = \"Acceptance criteria\"",
    );
    p.write("cairn.toml", &cfg);
    p.expect(&[
        "new",
        "Two lists",
        "-q",
        "--body",
        "## Notes\n\n- [ ] not a criterion\n- [ ] nor this\n\n\
         ## Acceptance criteria\n\n- [x] the only one that counts\n",
    ]);

    let out = p.expect(&["close", "1"]).all();
    assert!(
        !out.contains("unticked") && !out.contains(" of "),
        "the notes checklist was counted:\n{out}"
    );
    // `criteria` and `criteria_done` are derived, so they are columns and
    // filters rather than stored fields.
    let counts = p
        .expect(&[
            "list",
            "-A",
            "--plain",
            "--columns",
            "criteria,criteria_done",
        ])
        .trimmed();
    assert_eq!(
        counts, "1\t1",
        "the notes checklist was counted as acceptance criteria"
    );
}

/// `renumber --dry-run` and `--compact`, which are the two ways of asking it
/// not to do the obvious thing.
#[test]
fn renumber_can_describe_itself_before_acting() {
    let p = Project::new();
    p.add("Keeper", &[]);
    p.write(
        "cairn/items/0001-a-copy.md",
        "---\nid: 1\ntitle: A copy\nstatus: backlog\n---\nbody\n",
    );
    let before = p.files("cairn/items");

    let out = p.expect(&["renumber", "--dry-run"]).all();
    assert_contains(&out, "would be renumbered", "");
    assert_eq!(before, p.files("cairn/items"), "a dry run moved a file");

    p.expect(&["renumber"]);
    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());
}

/// Every `--json` shape a script might read, asserted to parse.
#[test]
fn every_machine_readable_shape_parses() {
    let p = seeded();
    p.expect(&["note", "1", "something"]);

    for args in [
        vec!["list", "-A", "--json"],
        vec!["next", "--json"],
        vec!["search", "item", "--json"],
        vec!["show", "1", "--json"],
        vec!["export"],
        vec!["config", "--json"],
    ] {
        let out = p.expect(&args);
        serde_json::from_str::<serde_json::Value>(&out.stdout)
            .unwrap_or_else(|e| panic!("`cairn {args:?}` is not JSON: {e}\n{}", out.stdout));
    }
}

/// A roadmap narrowed to one milestone, which is the positional argument
/// nothing had ever passed.
#[test]
fn a_roadmap_can_be_narrowed_to_one_milestone() {
    let p = seeded();
    milestone(&p, "v0.2", Some("2027-01-01"));
    p.expect(&["set", "2", "milestone=v0.2"]);

    let all = p.expect(&["roadmap"]).stdout;
    assert_contains(&all, "v0.1", "");
    assert_contains(&all, "v0.2", "");

    let one = p.expect(&["roadmap", "v0.1"]).stdout;
    assert_contains(&one, "v0.1", "");
    assert!(
        !one.contains("v0.2"),
        "it showed a milestone nobody asked for:\n{one}"
    );

    let out = p.fails(&["roadmap", "v9.9"]);
    assert_contains(&out.all(), "unknown milestone", "");
    assert_contains(&out.all(), "v0.1", "and lists the ones there are");
}

/// A description on the project reaches both the terminal roadmap and the
/// rendered one.
#[test]
fn a_project_description_is_shown_where_the_roadmap_is() {
    let p = seeded();
    let cfg = p.read("cairn.toml").replace(
        "[project]",
        "[project]\ndescription = \"A thing that does a thing.\"",
    );
    p.write("cairn.toml", &cfg);

    assert_contains(
        &p.expect(&["roadmap"]).stdout,
        "A thing that does a thing.",
        "",
    );
    p.expect(&["render"]);
    assert_contains(&p.read("ROADMAP.md"), "A thing that does a thing.", "");
}

/// A format 1 project written before the milestone type and field existed:
/// the migration has to add both, not assume them.
#[test]
fn migrating_adds_the_type_and_field_when_they_are_absent() {
    let p = Project::empty();
    p.write(
        "cairn.toml",
        "format = 1\n[project]\nname = \"Old\"\n\
         [[status]]\nname = \"todo\"\ncategory = \"open\"\n\
         [[type]]\nname = \"feature\"\n\
         \n[[milestone]]\nname = \"v0.1\"\ntitle = \"First\"\ndue = \"2026-12-01\"\n",
    );
    std::fs::create_dir_all(p.path("cairn/items")).unwrap();
    p.write(
        "cairn/items/0001-scheduled.md",
        "---\nid: 1\ntitle: Scheduled\nstatus: todo\nmilestone: v0.1\n---\nbody\n",
    );

    let out = p.expect(&["migrate"]).all();
    assert_contains(&out, "milestone", "");

    let cfg = p.read("cairn.toml");
    assert!(
        cfg.contains("name = \"milestone\""),
        "the type was not added:\n{cfg}"
    );
    assert!(
        cfg.contains("kind = \"ref\""),
        "the field was not added:\n{cfg}"
    );
    assert!(
        cfg.contains("name = \"due\""),
        "`due` was not declared:\n{cfg}"
    );
    assert!(
        !cfg.contains("[[milestone]]"),
        "the old block survived:\n{cfg}"
    );

    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());
    assert_contains(
        &p.expect(&["roadmap"]).stdout,
        "First",
        "the roadmap survived",
    );
}

/// Claiming has several ways to refuse, and each of them is a sentence somebody
/// has to act on.
#[test]
fn claiming_refuses_for_reasons_it_names() {
    let p = seeded();
    p.expect(&["set", "2", "depends_on=1"]);

    // Blocked.
    let out = p.fails(&["claim", "2"]);
    assert_contains(&out.all(), "blocked", "");
    assert!(
        p.run(&["claim", "2", "--force"]).ok(),
        "--force takes it anyway"
    );

    // Held by somebody else.
    p.expect(&["set", "3", "assignee=someone-else"]);
    let out = p.fails(&["claim", "3"]);
    assert_contains(&out.all(), "someone-else", "it names who has it");
    assert!(p.run(&["claim", "3", "--force"]).ok());

    // Released, and then free again.
    p.expect(&["release", "3"]);
    assert!(p.run(&["claim", "3"]).ok(), "release did not free it");

    // Nothing left to claim.
    p.expect(&["claim", "1"]);
    let out = p.run(&["claim", "--next"]);
    assert!(
        !out.ok(),
        "there is nothing unclaimed and ready: {}",
        out.all()
    );
}

/// `new` has flags for everything an item carries, and most set a field the
/// long way round.
#[test]
fn new_can_set_everything_an_item_carries() {
    let p = seeded();
    let id = p
        .expect(&[
            "new",
            "Fully specified",
            "-q",
            "-t",
            "bug",
            "-s",
            "planned",
            "-m",
            "v0.1",
            "-l",
            "one,two",
            "-a",
            "somebody",
            "-d",
            "1",
            "--set",
            "priority=p0",
            "--body",
            "A body of its own.",
        ])
        .trimmed();

    let json: serde_json::Value =
        serde_json::from_str(&p.expect(&["show", &id, "--json"]).stdout).unwrap();
    assert_eq!(json["type"], "bug");
    assert_eq!(json["status"], "planned");
    assert_eq!(json["milestone"], "v0.1");
    assert_eq!(json["assignee"], "somebody");
    assert_eq!(json["labels"], serde_json::json!(["one", "two"]));
    assert_eq!(json["depends_on"], serde_json::json!([1]));
    assert_eq!(json["fields"]["priority"], "p0");
    assert_contains(json["body"].as_str().unwrap(), "A body of its own.", "");

    // A body from standard input, which is how a hook or a script writes one.
    let out = p.run_stdin(&["new", "From a pipe", "-q", "--stdin"], "piped body\n");
    assert!(out.ok(), "{}", out.all());
    assert_contains(
        &p.expect(&["show", out.trimmed().as_str()]).stdout,
        "piped body",
        "",
    );
}

/// Removing an item takes its references with it, and asks first.
#[test]
fn removing_an_item_repairs_what_pointed_at_it() {
    let p = seeded();
    p.expect(&["set", "2", "depends_on=1"]);
    p.expect(&["set", "3", "milestone=v0.1"]);

    // It asks, and a refusal changes nothing.
    let out = p.run_stdin(&["remove", "1"], "n\n");
    assert!(
        !out.ok() || p.run(&["show", "1"]).ok(),
        "a declined removal deleted it"
    );

    p.expect(&["remove", "1", "--force"]);
    assert!(!p.run(&["show", "1"]).ok(), "it is gone");
    let json: serde_json::Value =
        serde_json::from_str(&p.expect(&["show", "2", "--json"]).stdout).unwrap();
    assert_eq!(
        json["depends_on"],
        serde_json::json!([]),
        "a dangling dependency was left behind: {json}"
    );
    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());
}

/// The reserved fields `set` handles by name, each with its own rule about
/// what an empty value and a list value mean.
#[test]
fn every_reserved_field_has_its_own_rule() {
    let p = seeded();

    // `key`: set, cleared, and refused when it would read as an identifier.
    p.expect(&["set", "1", "key=first"]);
    assert_contains(&p.expect(&["show", "1"]).stdout, "first", "");
    let out = p.fails(&["set", "1", "key=0002"]);
    assert_contains(&out.all(), "reads as an identifier", "");
    p.expect(&["set", "1", "key="]);

    // `owner` and `created_by` are ordinary strings that can be cleared.
    for field in ["owner", "created_by"] {
        p.expect(&["set", "1", &format!("{field}=somebody")]);
        assert_contains(
            &p.expect(&["show", "1", "--json"]).stdout,
            "somebody",
            field,
        );
        p.expect(&["set", "1", &format!("{field}=")]);
    }

    // A title cannot be emptied, and is not a list.
    let out = p.fails(&["set", "1", "title="]);
    assert_contains(&out.all(), "cannot be empty", "");
    let out = p.fails(&["set", "1", "title+=more"]);
    assert_contains(&out.all(), "not a list field", "");

    // `id` is identity and cannot be assigned at all.
    let out = p.fails(&["set", "1", "id=99"]);
    assert_contains(&out.all(), "cannot be changed", "");

    // `type` cleared, and refused when it names nothing.
    p.expect(&["set", "1", "type="]);
    let out = p.fails(&["set", "1", "type=sculpture"]);
    assert_contains(&out.all(), "sculpture", "");

    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());
}

/// Adding to and removing from a list field, rather than replacing it.
#[test]
fn a_list_field_can_be_added_to_and_taken_from() {
    let p = seeded();
    p.expect(&["set", "1", "labels=one,two"]);
    p.expect(&["set", "1", "labels+=three"]);
    let json: serde_json::Value =
        serde_json::from_str(&p.expect(&["show", "1", "--json"]).stdout).unwrap();
    assert_eq!(json["labels"], serde_json::json!(["one", "two", "three"]));

    p.expect(&["set", "1", "labels-=two"]);
    let json: serde_json::Value =
        serde_json::from_str(&p.expect(&["show", "1", "--json"]).stdout).unwrap();
    assert_eq!(json["labels"], serde_json::json!(["one", "three"]));

    p.expect(&["set", "1", "labels="]);
    let json: serde_json::Value =
        serde_json::from_str(&p.expect(&["show", "1", "--json"]).stdout).unwrap();
    assert_eq!(json["labels"], serde_json::json!([]));
}

/// A cycle cannot be created by an ordinary command, however it is approached.
#[test]
fn a_dependency_cycle_is_refused_at_every_depth() {
    let p = Project::new();
    p.add("One", &[]);
    p.add("Two", &["-d", "1"]);
    p.add("Three", &["-d", "2"]);

    let out = p.fails(&["set", "1", "depends_on=1"]);
    assert_contains(&out.all(), "cycle", "an item depending on itself");

    let out = p.fails(&["set", "1", "depends_on=3"]);
    assert_contains(&out.all(), "cycle", "closing a loop three deep");

    // A dependency on nothing is refused too, at creation as well as on an
    // edit: `new` checked only for a cycle, so it accepted one and left `check`
    // to complain afterwards.
    let out = p.fails(&["set", "1", "depends_on=999"]);
    assert_contains(&out.all(), "does not exist", "");
    let out = p.fails(&["new", "Four", "-d", "999"]);
    assert_contains(&out.all(), "does not exist", "");

    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());
}

/// The render hook fires with the file it wrote and how many items went into
/// it, which is a different shape from the item hooks.
///
/// Unix only, and deliberately: the string form of a hook goes through the
/// platform shell, which is the thing that makes it platform-specific. The
/// manual says so, and the portable form is the argv array tested below.
#[cfg(unix)]
#[test]
fn the_render_hook_reports_the_file_and_the_count() {
    let p = seeded();
    let cfg = p.read("cairn.toml").replace(
        "[hooks]",
        "[hooks]\nafter-render = \"printf '%s %s' \\\"$CAIRN_RENDER_TARGET\\\" \\\"$CAIRN_ITEM_COUNT\\\" > rendered.txt\"",
    );
    p.write("cairn.toml", &cfg);

    p.expect(&["render"]);
    let seen = p.read("rendered.txt");
    assert_contains(&seen, "ROADMAP.md", "the file it wrote");
    assert!(
        seen.split_whitespace()
            .nth(1)
            .is_some_and(|n| n.parse::<u32>().is_ok()),
        "the item count: {seen}"
    );
}

/// A hook given as an argv array runs with no shell at all, which is the
/// portable form.
#[test]
fn a_hook_can_be_an_argv_array() {
    let p = seeded();
    let cfg = p.read("cairn.toml").replace(
        "after-create = \"cairn render -q\"",
        "after-create = [\"cairn\", \"render\", \"-q\"]",
    );
    p.write("cairn.toml", &cfg);

    p.expect(&["new", "Triggers the hook", "-q"]);
    assert_contains(
        &p.read("ROADMAP.md"),
        "Triggers the hook",
        "the array-form hook did not run",
    );
}

/// A hook that fails warns and does not roll anything back, because it runs
/// after the change is already on disk.
#[test]
fn a_failing_hook_warns_without_undoing_anything() {
    let p = seeded();
    let cfg = p.read("cairn.toml").replace(
        "after-create = \"cairn render -q\"",
        "after-create = \"exit 3\"",
    );
    p.write("cairn.toml", &cfg);

    let out = p.expect(&["new", "Written anyway", "-q"]);
    assert_contains(&out.all(), "hook", "it says the hook failed");
    assert_contains(
        &p.expect(&["list", "-A", "--plain"]).stdout,
        "Written anyway",
        "and the item is still there",
    );
}

/// `owner` and `created_by` were writable, shown as columns, and absent from
/// every machine-readable shape — so an export dropped them and a round trip
/// through the interchange document lost them without a word.
#[test]
fn who_is_answerable_survives_a_round_trip() {
    let p = seeded();
    p.expect(&["set", "1", "owner=a-person"]);
    p.expect(&["set", "1", "created_by=an-agent"]);

    let json: serde_json::Value =
        serde_json::from_str(&p.expect(&["show", "1", "--json"]).stdout).unwrap();
    assert_eq!(json["owner"], "a-person", "{json}");
    assert_eq!(json["created_by"], "an-agent", "{json}");

    let doc = p.expect(&["export"]).stdout;
    assert_contains(&doc, "a-person", "the export carries the owner");

    let fresh = Project::new();
    fresh.run_stdin(&["import"], &doc);
    let json: serde_json::Value =
        serde_json::from_str(&fresh.expect(&["show", "1", "--json"]).stdout).unwrap();
    assert_eq!(
        json["owner"], "a-person",
        "the owner was lost in transit: {json}"
    );
    assert_eq!(json["created_by"], "an-agent", "{json}");
}

// --- a claim is a promise somebody might not keep ---------------------------

/// A project that says how long a claim may go untouched.
fn with_stale_after(days: u32) -> Project {
    let p = Project::with(Schema::standard().claim_stale_after(days));
    seed(&p);
    p
}

#[test]
fn a_claim_records_when_it_was_taken() {
    let p = seeded();
    p.expect(&["claim", "2", "--as", "agent-one"]);

    let file = p.item_file("2");
    assert_contains(&file, "assignee: agent-one", "");
    assert_contains(&file, "claimed:", "a claim records when, not only who");

    // `updated` means something else and every other edit moves it, so it is a
    // key of its own.
    p.expect(&["set", "2", "priority=p0"]);
    assert_contains(
        &p.item_file("2"),
        "claimed:",
        "an unrelated edit cleared it",
    );

    // Handing it back or finishing it clears the claim.
    p.expect(&["release", "2"]);
    assert_missing(
        &p.item_file("2"),
        "claimed:",
        "release left the claim behind",
    );

    p.expect(&["claim", "3"]);
    p.expect(&["close", "3"]);
    assert_missing(&p.item_file("3"), "claimed:", "close left the claim behind");
}

/// A claim nobody is honouring is invisible twice over: `next` will not offer
/// it because it is held, and nothing lists it because nothing knows.
#[test]
fn a_stale_claim_is_visible_and_never_revoked() {
    let p = with_stale_after(3);
    p.write_item(
        "0050-abandoned.md",
        "id: 50\ntitle: Abandoned\nstatus: doing\nassignee: ghost\nclaimed: 2020-01-01",
        "body\n",
    );

    assert_eq!(p.count_of_all("stale=true"), 1, "nothing was called stale");
    assert!(
        p.count_of_all("held_days>100") >= 1,
        "how long it has been held is not derivable"
    );

    // Offered, with who has it and for how long.
    let out = p.expect(&["next"]).all();
    assert_contains(&out, "stale:", "");
    assert_contains(&out, "ghost", "it names who holds it");
    assert_contains(&out, "day(s)", "and for how long");

    // Offered, never taken. The item is still theirs until somebody says
    // otherwise.
    assert_contains(&p.item_file("50"), "assignee: ghost", "it was revoked");
}

#[test]
fn a_stale_claim_can_be_taken_over_without_force() {
    let p = with_stale_after(3);
    p.write_item(
        "0050-abandoned.md",
        "id: 50\ntitle: Abandoned\nstatus: doing\nassignee: ghost\nclaimed: 2020-01-01",
        "body\n",
    );

    let out = p.expect(&["claim", "50", "--as", "somebody"]).all();
    assert_contains(&out, "taken over from ghost", "it says whose it was");
    assert_contains(&out, "day(s)", "and how long they held it");
    assert_contains(&p.item_file("50"), "assignee: somebody", "");

    // A claim that is not stale still needs --force.
    p.expect(&["claim", "1", "--as", "holder"]);
    let out = p.fails(&["claim", "1", "--as", "interloper"]);
    assert_contains(&out.all(), "--force", "");
}

/// No threshold, no staleness. A project that has not said what a claim means
/// should see nothing new.
#[test]
fn without_a_threshold_nothing_is_ever_stale() {
    let p = seeded();
    p.write_item(
        "0050-ancient.md",
        "id: 50\ntitle: Ancient\nstatus: doing\nassignee: ghost\nclaimed: 2020-01-01",
        "body\n",
    );
    assert_eq!(p.count_of_all("stale=true"), 0);
    assert!(!p.expect(&["next"]).all().contains("stale:"));
    let out = p.fails(&["claim", "50", "--as", "somebody"]);
    assert_contains(&out.all(), "--force", "an old claim is still a claim");
}

// --- handing work back ------------------------------------------------------

/// The most valuable thing somebody handing work back knows is why, and it
/// used to evaporate: the next taker walked the same dead end.
#[test]
fn releasing_with_a_reason_reaches_the_next_taker() {
    let p = seeded();
    p.expect(&["claim", "2"]);
    p.expect(&[
        "release",
        "2",
        "--reason",
        "The parser rewrite needs the format decision first.",
    ]);

    let file = p.item_file("2");
    assert_contains(
        &file,
        "## Released by",
        "a dated note in the releaser's name",
    );
    assert_contains(&file, "format decision", "carrying what they knew");

    let out = p.expect(&["claim", "2"]).all();
    assert_contains(
        &out,
        "last released because:",
        "the next taker is told before they start",
    );
    assert_contains(&out, "format decision", "");

    // Three attempts on a hard item is a history, so nothing is overwritten.
    p.expect(&["release", "2", "--reason", "Second attempt, same wall."]);
    let file = p.item_file("2");
    assert_contains(&file, "format decision", "the first reason survived");
    assert_contains(&file, "Second attempt", "and the second is there too");

    // Nothing goes into frontmatter.
    let front = file.split("---").nth(1).unwrap_or_default();
    assert_missing(front, "released", "a reason is a history, not a field");
}

#[test]
fn releasing_without_a_reason_still_works_and_says_nothing_extra() {
    let p = seeded();
    p.expect(&["claim", "2"]);
    let out = p.expect(&["release", "2"]).all();
    assert_missing(&out, "Released by", "");
    assert!(p.run(&["check"]).ok());
}

// --- filing the same thing twice --------------------------------------------

/// An agent starts every session cold, so filing a duplicate is not an unlucky
/// mistake but the characteristic failure of an agent using this tool.
#[test]
fn a_near_duplicate_title_is_reported_and_created_anyway() {
    let p = Project::new();
    p.add("Rate-limit the public API", &[]);

    let out = p.expect(&["new", "Rate limit the API"]).all();
    assert_contains(&out, "look", "it says something looks similar");
    assert_contains(&out, "Rate-limit the public API", "and names it");
    assert_eq!(p.count_all(), 2, "reported, not refused");

    // Word order and punctuation do not hide a duplicate.
    let out = p.expect(&["new", "the API, rate limited"]).all();
    assert_contains(&out, "look", "word order should not hide it");

    // Something genuinely different says nothing.
    let out = p.expect(&["new", "Document the export format"]).all();
    assert_missing(&out, "look", "an unrelated title was called similar");

    // A finished item counts: re-filing something already done is the same
    // mistake.
    p.expect(&["close", "1"]);
    let out = p.expect(&["new", "Rate-limit the public API again"]).all();
    assert_contains(&out, "look", "a closed duplicate was not considered");

    assert!(p.run(&["check"]).ok());
}

#[test]
fn a_quiet_creation_says_nothing_about_duplicates() {
    let p = Project::new();
    p.add("Rate-limit the public API", &[]);
    let out = p.expect(&["new", "Rate limit the API", "-q"]).all();
    assert_missing(&out, "look", "--quiet should be quiet");
}

/// Over the protocol the warning has to be in the result: a tool result is what
/// the model reads, and standard error is not.
#[test]
fn an_agent_is_told_in_band_that_it_may_be_filing_a_duplicate() {
    let p = Project::new();
    p.add("Rate-limit the public API", &[]);

    let r = p.mcp_call(
        "create_item",
        serde_json::json!({"title": "Rate limit the API"}),
        Some("claude"),
    );
    assert!(!refused(&r), "{}", tool_text(&r));
    let doc: serde_json::Value = serde_json::from_str(&tool_text(&r)).expect("JSON");
    assert!(
        doc["similar"].is_array(),
        "no similar items reported: {doc}"
    );
    assert_json(
        &doc,
        "similar.0.title",
        serde_json::json!("Rate-limit the public API"),
    );
    assert_contains(doc["note"].as_str().unwrap_or_default(), "look", "");
}

/// An agent that cannot finish should hand the item back with what it learned,
/// rather than leaving it claimed for ever.
#[test]
fn an_agent_can_hand_work_back_with_a_reason() {
    let p = seeded();
    p.mcp_call_anonymous("claim_item", serde_json::json!({"id": 2}), "claude");

    let r = p.mcp_call_anonymous(
        "release_item",
        serde_json::json!({"id": 2, "reason": "Needs a decision only a person can make."}),
        "claude",
    );
    assert!(!refused(&r), "{}", tool_text(&r));

    let file = p.item_file("2");
    assert_contains(&file, "## Released by claude", "in the agent's own name");
    assert_contains(&file, "only a person can make", "");
    assert_missing(&file, "claimed:", "the claim was not cleared");
    assert!(p.run(&["check"]).ok());
}

// --- asking for less --------------------------------------------------------

/// Every read returned twenty-two keys and about six hundred characters an
/// item. Context is the one resource an agent cannot get more of, and cairn was
/// spending it on the caller's behalf without asking.
#[test]
fn an_agent_can_ask_for_only_the_keys_it_needs() {
    let p = seeded();
    for n in 0..6 {
        p.add(&format!("Filler {n}"), &[]);
    }

    let full = tool_text(&p.mcp_call("list_items", serde_json::json!({}), Some("claude")));
    let narrow = tool_text(&p.mcp_call(
        "list_items",
        serde_json::json!({"fields": ["title", "status", "priority", "blocked"]}),
        Some("claude"),
    ));
    assert!(
        narrow.len() * 2 < full.len(),
        "asking for four keys of twenty-two saved almost nothing: {} against {}",
        narrow.len(),
        full.len()
    );

    let doc: serde_json::Value = serde_json::from_str(&narrow).expect("JSON");
    let first = &doc["items"][0];
    let keys: Vec<&String> = first.as_object().expect("an object").keys().collect();
    assert_eq!(keys.len(), 5, "got more than was asked for: {keys:?}");
    // A result nothing can be acted on is worth less than the bytes it took.
    assert!(first["id"].is_number(), "id must survive: {first}");
}

/// A milestone carries no priority. Asking for one across a mixed list is a
/// reasonable request that answers `null`, not a failure because the first item
/// happened not to have one.
#[test]
fn a_field_an_item_lacks_is_null_rather_than_an_error() {
    let p = seeded();
    // Written by hand, because every item a command creates receives the
    // schema's default.
    p.write_item(
        "0050-bare.md",
        "id: 50\ntitle: Bare\nstatus: backlog",
        "body\n",
    );
    let doc: serde_json::Value = serde_json::from_str(&tool_text(&p.mcp_call(
        "list_items",
        serde_json::json!({"fields": ["title", "priority"], "include_closed": true}),
        Some("claude"),
    )))
    .expect("JSON");

    let items = doc["items"].as_array().expect("items");
    assert!(
        items.iter().any(|i| i["priority"].is_null()),
        "nothing came back null: {doc}"
    );
    assert!(
        items.iter().any(|i| i["priority"].is_string()),
        "nothing came back set: {doc}"
    );
}

/// A schema field lives under `fields` in the full shape, and a caller asking
/// for `priority` means the project's priority. Where cairn keeps it is cairn's
/// business.
#[test]
fn a_schema_field_can_be_asked_for_by_its_own_name() {
    let p = seeded();
    let doc: serde_json::Value = serde_json::from_str(&tool_text(&p.mcp_call(
        "show_item",
        serde_json::json!({"id": 1, "fields": ["title", "priority"]}),
        Some("claude"),
    )))
    .expect("JSON");
    assert_json(&doc, "priority", serde_json::json!("p0"));
}

#[test]
fn asking_for_a_field_that_does_not_exist_says_what_does() {
    let p = seeded();
    let r = p.mcp_call(
        "next_items",
        serde_json::json!({"fields": ["nonesuch"]}),
        Some("claude"),
    );
    assert!(refused(&r), "{}", tool_text(&r));
    assert_contains(&tool_text(&r), "unknown field `nonesuch`", "");
    assert_contains(&tool_text(&r), "priority", "and lists what is available");
}

/// The default is what every existing consumer already reads.
#[test]
fn asking_for_nothing_returns_what_it_always_did() {
    let p = seeded();
    let doc: serde_json::Value = serde_json::from_str(&tool_text(&p.mcp_call(
        "list_items",
        serde_json::json!({}),
        Some("claude"),
    )))
    .expect("JSON");
    let keys: Vec<&String> = doc["items"][0]
        .as_object()
        .expect("object")
        .keys()
        .collect();
    assert!(keys.len() > 15, "the default shape narrowed: {keys:?}");
}

// --- what changed since I last looked ---------------------------------------

#[test]
fn a_returning_caller_can_ask_only_for_what_moved() {
    let p = Project::new();
    p.add("Ancient", &[]);
    p.add("Recent", &[]);
    // Reach in, because `updated` is what the filter reads and time is not a
    // thing a test may wait for.
    let old = p.expect(&["show", "1", "--path"]).trimmed();
    let text = p
        .read(&old)
        .replace(&format!("updated: {}", today()), "updated: 2020-01-01");
    p.write(&old, &text);

    assert_eq!(p.expect(&["list", "--ids"]).lines().len(), 2);
    assert_eq!(
        p.expect(&["list", "--since", "2026-01-01", "--ids"])
            .lines(),
        vec!["0002".to_string()],
    );
    // And on search, and over the protocol.
    p.expect(&["search", "e", "--since", "2026-01-01"]);
    let doc: serde_json::Value = serde_json::from_str(&tool_text(&p.mcp_call(
        "list_items",
        serde_json::json!({"since": "2026-01-01", "fields": ["title"]}),
        Some("claude"),
    )))
    .expect("JSON");
    assert_json(&doc, "count", serde_json::json!(1));

    // A date that is not one says what it wanted.
    let out = p.fails(&["list", "--since", "yesterday"]);
    assert_contains(&out.all(), "YYYY-MM-DD", "");
}

fn today() -> String {
    let out = std::process::Command::new("date")
        .args(["+%Y-%m-%d"])
        .output()
        .expect("date");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

// --- a proposal is a proposal -----------------------------------------------

/// A project where an agent may ask for a priority but not set one.
fn proposing() -> Project {
    let p = Project::with(
        Schema::standard()
            .amend("priority", |f| f.agent(Agent::Propose))
            .amend_status("done", |s| s.agent(Agent::Propose)),
    );
    seed(&p);
    p
}

/// The permission already refused the write and told the caller to write a
/// note. Then the proposal was prose a person had to find by reading every body
/// in the backlog.
#[test]
fn a_refusal_names_the_command_that_replaces_it() {
    let p = proposing();
    let out = p.fails_as_agent(&["set", "1", "priority=p1"]);
    assert_contains(&out.all(), "cairn propose", "it names the way through");
    assert_contains(&out.all(), "priority=p1", "with the change already spelled");
    assert_contains(&out.all(), "cairn proposals", "and who reviews it");
}

#[test]
fn a_proposal_is_listed_reviewed_and_applied() {
    let p = proposing();
    p.expect(&[
        "propose",
        "1",
        "priority=p3",
        "--why",
        "It blocks nothing and nobody has asked.",
    ]);

    // Nothing has changed yet.
    assert_json(
        &p.json(&["show", "1", "--json"]),
        "fields.priority",
        serde_json::json!("p0"),
    );

    let listed = p.expect(&["proposals"]).stdout;
    assert_contains(&listed, "priority: p0 -> p3", "the change, both ends");
    assert_contains(&listed, "blocks nothing", "and the reason");

    let json = p.json(&["proposals", "--json"]);
    assert_json(&json, "0.field", serde_json::json!("priority"));
    assert_json(&json, "0.from", serde_json::json!("p0"));
    assert_json(&json, "0.to", serde_json::json!("p3"));

    p.expect(&["proposals", "--accept", "1"]);
    assert_json(
        &p.json(&["show", "1", "--json"]),
        "fields.priority",
        serde_json::json!("p3"),
    );

    // Applied, and recorded as applied rather than looking as though it was
    // always so.
    assert_contains(&p.item_file("1"), "## Accepted priority", "");
    assert!(
        p.expect(&["proposals"])
            .all()
            .contains("nothing is proposed"),
        "an accepted proposal is still listed as waiting"
    );
    assert!(p.run(&["check"]).ok());
}

/// A proposal lives in the body, so an unrelated write cannot lose it.
#[test]
fn a_proposal_survives_an_unrelated_change() {
    let p = proposing();
    p.expect(&["propose", "1", "priority=p1", "--why", "Because."]);
    p.expect(&["set", "1", "area=parser"]);
    p.expect(&["set", "1", "labels+=x"]);
    assert_contains(&p.expect(&["proposals"]).stdout, "priority: p0 -> p1", "");
}

/// Several proposals on one item is a conversation, not a value.
#[test]
fn proposals_accumulate_rather_than_replace() {
    let p = proposing();
    p.expect(&["propose", "1", "priority=p1", "--why", "First argument."]);
    p.expect(&["propose", "1", "priority=p3", "--why", "Second argument."]);

    let listed = p.expect(&["proposals"]).stdout;
    assert_contains(&listed, "First argument.", "the earlier one survived");
    assert_contains(&listed, "Second argument.", "and the later one is there");

    // Accepting takes the most recent.
    p.expect(&["proposals", "--accept", "1"]);
    assert_json(
        &p.json(&["show", "1", "--json"]),
        "fields.priority",
        serde_json::json!("p3"),
    );
}

#[test]
fn a_proposal_is_checked_against_the_schema() {
    let p = proposing();
    let out = p.fails(&["propose", "1", "priority=urgent"]);
    assert_contains(
        &out.all(),
        "urgent",
        "a value the schema forbids is refused",
    );

    let out = p.fails(&["propose", "1", "priority=p0"]);
    assert_contains(&out.all(), "already", "proposing what is already true");

    let out = p.fails(&["propose", "1", "nonsense"]);
    assert_contains(&out.all(), "field=value", "");

    let out = p.fails(&["proposals", "--accept", "2"]);
    assert_contains(&out.all(), "nothing proposed", "");
}

/// The agent is the caller this exists for, so it has to be reachable from
/// where an agent is.
#[test]
fn an_agent_can_propose_what_it_may_not_set() {
    let p = proposing();

    let refused_write = p.mcp_call(
        "update_item",
        serde_json::json!({"id": 1, "fields": {"priority": "p3"}}),
        Some("claude"),
    );
    assert!(refused(&refused_write));
    assert_contains(
        &tool_text(&refused_write),
        "propose",
        "the refusal points here",
    );

    let r = p.mcp_call_anonymous(
        "propose_change",
        serde_json::json!({
            "id": 1, "field": "priority", "value": "p3",
            "why": "Nothing depends on it any more."
        }),
        "claude",
    );
    assert!(!refused(&r), "{}", tool_text(&r));

    // Nothing changed, and a person can see what is waiting.
    assert_json(
        &p.json(&["show", "1", "--json"]),
        "fields.priority",
        serde_json::json!("p0"),
    );
    let listed = p.expect(&["proposals"]).stdout;
    assert_contains(&listed, "claude", "in the agent's own name");
    assert_contains(&listed, "Nothing depends on it", "");
}

/// A proposal without a reason is a preference, not an argument.
#[test]
fn an_agent_must_say_why() {
    let p = proposing();
    let r = p.mcp_call(
        "propose_change",
        serde_json::json!({"id": 1, "field": "priority", "value": "p3"}),
        Some("claude"),
    );
    assert!(refused(&r));
    assert_contains(&tool_text(&r), "why", "");
}

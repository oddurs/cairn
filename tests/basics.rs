// cairn — end-to-end tests: basics.
//
// Copyright (c) 2026 Oddur Sigurdsson. MIT licensed; see LICENSE.
//
// Creating, reading, changing and validating items — the commands somebody
// meets first.
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
    let replies = p.mcp(
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
    let replies = p.mcp(
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

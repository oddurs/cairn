// cairn — end-to-end tests: schema.
//
// Copyright (c) 2026 Oddur Sigurdsson. MIT licensed; see LICENSE.
//
// What a project may declare about its own items, and what cairn does with it:
// types, fields, references, milestones, identifiers, criteria.
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
    // A schema where nothing declares `groups`: internally consistent, every
    // reference resolving, and no roadmap to draw.
    let p = Project::with(Schema::standard().amend_type("milestone", |t| t.groups_none()));

    let out = p.expect(&["roadmap"]).all();
    assert_contains(&out, "no type declares `groups`", "it says what is missing");
    assert_contains(&out, "groups = \"one\"", "and what to write instead");

    // The same defect arriving by the other road.
    assert_contains(
        &p.expect(&["check"]).all(),
        "no [[type]] declares `groups`",
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
    let cfg = old.read("cairn.toml").replace("format = 3", "format = 1");
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

// --- types that group work --------------------------------------------------

/// The whole point: one declaration, and the field it implies exists.
#[test]
fn declaring_that_a_type_groups_work_creates_its_field() {
    let p = Project::with(
        Schema::bare()
            .item_type(ItemType::new("task"))
            .item_type(ItemType::new("release").groups_one().inverse("scheduled"))
            .item_type(ItemType::new("epic").groups_many().inverse("contains")),
    );
    p.expect(&["new", "First release", "-t", "release", "-q"]);
    p.expect(&["set", "1", "key=v1"]);
    p.expect(&["new", "Auth rework", "-t", "epic", "-q"]);
    p.expect(&["set", "2", "key=auth"]);
    p.expect(&["new", "Login page", "-t", "task", "-q"]);

    // No `[[field]]` declares either of these.
    p.expect(&["set", "3", "release=v1", "epic=auth"]);

    let file = p.item_file("3");
    assert_contains(&file, "release: v1", "single-valued, so a scalar");
    assert_contains(&file, "epic:\n- auth", "many-valued, so a sequence");
    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());
}

/// Grouping types are what work is filed under, so they are not work.
#[test]
fn a_grouping_type_is_absent_from_what_can_be_started() {
    let p = Project::with(
        Schema::bare()
            .item_type(ItemType::new("task"))
            .item_type(ItemType::new("release").groups_one()),
    );
    p.expect(&["new", "v1", "-t", "release", "-q"]);
    p.expect(&["set", "1", "key=v1"]);
    p.expect(&["new", "Real work", "-t", "task", "-q"]);

    assert_eq!(
        p.expect(&["next", "--ids"]).lines(),
        vec!["0002".to_string()]
    );
    assert!(
        !p.expect(&["board"]).stdout.contains("v1"),
        "a container is a column heading, not a card"
    );
}

/// The name is not the mechanism. A project may call it whatever it likes and
/// everything keeps working, because cairn looks for the structural fact.
#[test]
fn the_schedule_type_need_not_be_called_milestone() {
    let p = Project::with(
        Schema::bare()
            .item_type(ItemType::new("task"))
            .item_type(ItemType::new("release").groups_one())
            .render(|r| r.group_by("release")),
    );
    p.expect(&["new", "Version one", "-t", "release", "-q"]);
    p.expect(&["set", "1", "key=v1"]);
    p.expect(&["new", "Some work", "-t", "task", "-q"]);
    p.expect(&["set", "2", "release=v1"]);

    let out = p.expect(&["roadmap"]).stdout;
    assert_contains(&out, "v1", "the roadmap found the schedule type");
    assert_contains(&out, "0/1", "and counted what is filed under it");

    assert_contains(
        &p.expect(&["board", "--group-by", "release"]).stdout,
        "v1",
        "",
    );
    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());
}

/// `cairn config` used to show nothing about the most consequential fact in a
/// schema.
#[test]
fn config_says_which_types_group_work() {
    let p = Project::with(
        Schema::bare()
            .item_type(ItemType::new("task"))
            .item_type(ItemType::new("release").groups_one())
            .item_type(ItemType::new("epic").groups_many()),
    );
    let shown = p.expect(&["config"]).stdout;
    assert_contains(&shown, "one per item", "the single-valued one says so");
    assert_contains(&shown, "several per item", "and the many-valued one");

    let json = p.json(&["config", "--json"]);
    let types = json["types"].as_array().expect("types");
    let release = types
        .iter()
        .find(|t| t["name"] == "release")
        .expect("release");
    assert_json(release, "groups", serde_json::json!("one"));
    let task = types.iter().find(|t| t["name"] == "task").expect("task");
    assert_json(task, "groups", serde_json::json!(null));
}

/// A general reference is still a `[[field]]`, and still points anywhere.
#[test]
fn a_general_reference_is_still_a_field() {
    let p =
        Project::with(Schema::standard().field(Field::reference("related", "*").by_id().many()));
    seed(&p);
    p.expect(&["set", "2", "related=1,3"]);
    assert_contains(&p.item_file("2"), "- 1", "");
    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());

    // And pointing at nothing is still refused.
    let out = p.fails(&["set", "2", "related=999"]);
    assert_contains(&out.all(), "does not exist", "");
}

/// Format 3 moves the declaration and touches no item.
#[test]
fn migrating_to_format_three_changes_no_item_file() {
    // Seeded at the current format, then the schema is rewritten to the way
    // format 2 said it — because a format 2 project cannot be written to, which
    // is the point of the version.
    let p = Project::with(Schema::standard());
    seed(&p);
    p.set_schema(
        Schema::standard()
            .format(2)
            .amend_type("milestone", |t| t.groups_none())
            .field(
                Field::reference("milestone", "milestone")
                    .by_key()
                    .rollup()
                    .inverse("scheduled"),
            ),
    );
    let before: Vec<String> = p
        .files("cairn/items")
        .iter()
        .map(|f| p.read(&format!("cairn/items/{f}")))
        .collect();

    let out = p.expect(&["migrate"]).all();
    assert_contains(&out, "groups work", "it says what it did");

    let after: Vec<String> = p
        .files("cairn/items")
        .iter()
        .map(|f| p.read(&format!("cairn/items/{f}")))
        .collect();
    assert_eq!(before, after, "an item file changed");

    let cfg = p.read("cairn.toml");
    assert_contains(&cfg, "groups = \"one\"", "the type says it now");
    assert!(
        !cfg.contains("target = \"milestone\""),
        "the field that used to say it is gone:\n{cfg}"
    );
    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());

    // And the roadmap still reads the same backlog.
    assert_contains(&p.expect(&["roadmap"]).stdout, "v0.1", "");
}

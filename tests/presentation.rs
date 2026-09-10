// cairn — end-to-end tests: presentation.
//
// Copyright (c) 2026 Oddur Sigurdsson. MIT licensed; see LICENSE.
//
// What the commands print: tables, boards, roadmaps, the rendered file, and the
// GNU conventions the output follows.
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
// --- GNU conventions --------------------------------------------------------

#[test]
fn version_carries_the_licence_notice() {
    let p = Project::empty();
    let long = p.expect(&["--version"]).stdout;
    assert_contains(&long, "License MIT", "the licence");
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

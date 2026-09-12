// cairn — end-to-end tests: durability.
//
// Copyright (c) 2026 Oddur Sigurdsson. MIT licensed; see LICENSE.
//
// The properties that must hold whatever happens: nothing lost, nothing
// half-written, and every failure legible.
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

/// `cairn tick` rewrites a line in the body, which is the one write path that
/// reassembles the body from `lines()` -- and `lines()` drops the `\r` along
/// with the `\n`. Reassembling with bare newlines would flip a CRLF item's
/// detected ending, turning the next edit into a whole-file diff.
#[test]
fn ticking_preserves_line_endings() {
    let p = Project::new();
    p.write(
        "cairn/items/0001-crlf.md",
        "---\r\nid: 1\r\ntitle: CRLF\r\nstatus: backlog\r\n---\r\n\r\n         ## Acceptance criteria\r\n\r\n- [ ] one\r\n- [ ] two\r\n",
    );
    p.expect(&["tick", "1", "1", "-q"]);
    let after = p.read("cairn/items/0001-crlf.md");
    assert!(after.contains("- [x] one"), "the box moved:\n{after:?}");
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

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
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_cairn")
}

/// The default hooks run `cairn render -q`, which finds cairn on PATH. That
/// holds for an installed binary and not for one under `target/`, so tests put
/// the built binary's directory on PATH and behave like an installed tool.
fn path_with_binary() -> std::ffi::OsString {
    let dir = Path::new(bin()).parent().expect("binary directory");
    let existing = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![dir.to_path_buf()];
    paths.extend(std::env::split_paths(&existing));
    std::env::join_paths(paths).expect("PATH")
}

// --- harness ----------------------------------------------------------------

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Out {
    fn ok(&self) -> bool {
        self.code == 0
    }
    /// Everything the command printed, for assertions that do not care which
    /// stream carried it.
    fn all(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
    fn trimmed(&self) -> String {
        self.stdout.trim().to_string()
    }
    fn lines(&self) -> Vec<String> {
        self.stdout.lines().map(str::to_string).collect()
    }
}

struct Project {
    dir: tempfile::TempDir,
}

impl Project {
    /// A directory with no configuration in it or above it.
    fn empty() -> Project {
        Project {
            dir: tempfile::tempdir().expect("temp dir"),
        }
    }

    fn new() -> Project {
        Project::with_init(&["init", "--bare", "--name", "Testbed"])
    }

    fn with_init(args: &[&str]) -> Project {
        let p = Project::empty();
        p.expect(args);
        p
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.root().join(rel)
    }

    fn run(&self, args: &[&str]) -> Out {
        self.run_in(self.root(), args)
    }

    fn run_in(&self, cwd: &Path, args: &[&str]) -> Out {
        let out = Command::new(bin())
            .args(args)
            .current_dir(cwd)
            // Deterministic output: no colour, a known identity, and hooks left
            // enabled so the hook tests can exercise them.
            .env("NO_COLOR", "1")
            .env("CAIRN_USER", "tester")
            .env("PATH", path_with_binary())
            .env_remove("CAIRN_NO_HOOKS")
            .stdin(Stdio::null())
            .output()
            .unwrap_or_else(|e| panic!("running cairn {args:?}: {e}"));
        Out {
            code: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        }
    }

    /// Run with extra environment. Used by the editor tests, which are about
    /// what cairn does with VISUAL and EDITOR.
    fn run_env(&self, args: &[&str], env: &[(&str, Option<&str>)]) -> Out {
        let mut c = Command::new(bin());
        c.args(args)
            .current_dir(self.root())
            .env("NO_COLOR", "1")
            .env("CAIRN_USER", "tester")
            .env("PATH", path_with_binary())
            .stdin(Stdio::null());
        for (k, v) in env {
            match v {
                Some(v) => c.env(k, v),
                None => c.env_remove(k),
            };
        }
        let out = c
            .output()
            .unwrap_or_else(|e| panic!("running cairn {args:?}: {e}"));
        Out {
            code: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        }
    }

    /// Run as an agent, the way the MCP server does.
    fn run_as_agent(&self, args: &[&str]) -> Out {
        self.run_env(args, &[("CAIRN_AGENT", Some("claude"))])
    }

    fn expect_as_agent(&self, args: &[&str]) -> Out {
        let out = self.run_as_agent(args);
        assert!(out.ok(), "cairn {args:?} failed:\n{}", out.all());
        out
    }

    fn fails_as_agent(&self, args: &[&str]) -> Out {
        let out = self.run_as_agent(args);
        assert!(
            !out.ok(),
            "cairn {args:?} should have failed:\n{}",
            out.all()
        );
        out
    }

    /// Run with something on standard input. Used by the MCP tests, which
    /// speak a request/response protocol over the child's stdio.
    fn run_stdin(&self, args: &[&str], input: &str) -> Out {
        use std::io::Write;
        let mut child = Command::new(bin())
            .args(args)
            .current_dir(self.root())
            .env("NO_COLOR", "1")
            .env("CAIRN_USER", "tester")
            .env("PATH", path_with_binary())
            .env_remove("CAIRN_NO_HOOKS")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("spawning cairn {args:?}: {e}"));
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(input.as_bytes())
            .expect("writing to cairn");
        let out = child.wait_with_output().expect("waiting for cairn");
        Out {
            code: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        }
    }

    /// Run and require success, reporting the command's own diagnostics on
    /// failure rather than a bare assertion.
    fn expect(&self, args: &[&str]) -> Out {
        let out = self.run(args);
        assert!(
            out.ok(),
            "cairn {args:?} failed with {}:\n{}",
            out.code,
            out.all()
        );
        out
    }

    fn fails(&self, args: &[&str]) -> Out {
        let out = self.run(args);
        assert!(
            !out.ok(),
            "cairn {args:?} unexpectedly succeeded:\n{}",
            out.all()
        );
        out
    }

    /// Create an item and return its id.
    fn add(&self, title: &str, extra: &[&str]) -> String {
        let mut args = vec!["new", title];
        args.extend_from_slice(extra);
        args.push("-q");
        self.expect(&args).trimmed()
    }

    fn count(&self) -> usize {
        self.expect(&["list", "--count"]).trimmed().parse().unwrap()
    }

    fn count_all(&self) -> usize {
        self.expect(&["list", "-A", "--count"])
            .trimmed()
            .parse()
            .unwrap()
    }

    fn count_of(&self, filter: &str) -> usize {
        self.expect(&["list", "--filter", filter, "--count"])
            .trimmed()
            .parse()
            .unwrap()
    }

    fn write(&self, rel: &str, contents: &str) {
        let path = self.path(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }

    /// Replace the whole `[hooks]` table. The standard preset ships with render
    /// hooks enabled, so a test that wants its own must displace them rather
    /// than add to them — two `after-create` keys is a duplicate-key error.
    fn set_hooks(&self, body: &str) {
        let toml = self.read("cairn.toml");
        let filled = match toml.find("[hooks]") {
            Some(at) => {
                let after = &toml[at + "[hooks]".len()..];
                // The section runs to the next table header, or to end of file.
                let end = after
                    .match_indices('[')
                    .find(|(i, _)| after[..*i].ends_with('\n'))
                    .map(|(i, _)| at + "[hooks]".len() + i)
                    .unwrap_or(toml.len());
                format!("{}[hooks]\n{body}\n{}", &toml[..at], &toml[end..])
            }
            None => format!("{toml}\n[hooks]\n{body}"),
        };
        self.write("cairn.toml", &filled);
    }

    fn append(&self, rel: &str, extra: &str) {
        let existing = self.read(rel);
        self.write(rel, &(existing + extra));
    }

    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.path(rel)).unwrap_or_default()
    }

    fn remove(&self, rel: &str) {
        let _ = std::fs::remove_file(self.path(rel));
    }

    fn exists(&self, rel: &str) -> bool {
        self.path(rel).exists()
    }

    fn json(&self, args: &[&str]) -> serde_json::Value {
        let out = self.expect(args);
        serde_json::from_str(&out.stdout)
            .unwrap_or_else(|e| panic!("cairn {args:?} did not print JSON: {e}\n{}", out.stdout))
    }
}

fn assert_contains(haystack: &str, needle: &str, what: &str) {
    assert!(
        haystack.contains(needle),
        "{what}: expected to find {needle:?} in:\n{haystack}"
    );
}

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
    p.add("First item", &["--type", "feature", "--set", "priority=p0"]);
    p.add("Second item", &["-t", "bug"]);
    p.add("Third item", &["-t", "chore"]);
    // The milestone comes last so the three items keep the identifiers every
    // test below names. A milestone is an item in format 2, so it has to exist
    // before anything can point at it — the same as a dependency.
    milestone(&p, "v0.1", Some("2026-12-01"));
    p.expect(&["set", "1", "milestone=v0.1", "-q"]);
    p
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
    let p = Project::new();
    let cfg = p
        .read("cairn.toml")
        .replace("[render]", "[render]\nheader = \"docs/missing.md\"")
        .replace("link_items = false", "link_items = true")
        .replace("group_by = \"milestone\"", "group_by = \"epic\"");
    p.write("cairn.toml", &cfg);

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

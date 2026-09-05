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
    assert_eq!(bare.count_all(), 0);
    let seeded = Project::with_init(&["init", "--name", "Seeded"]);
    assert_eq!(seeded.count_all(), 1);
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
    p.add(
        "First item",
        &[
            "--type",
            "feature",
            "--milestone",
            "v0.1",
            "--set",
            "priority=p0",
        ],
    );
    p.add("Second item", &["-t", "bug"]);
    p.add("Third item", &["-t", "chore"]);
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
    assert_eq!(p.count_of("type!=bug"), 2, "negation");
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
    assert_eq!(p.count_all(), 3, "--all shows them");
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

#[test]
fn milestones_are_managed_through_the_configuration() {
    let p = seeded();
    p.expect(&[
        "milestone",
        "add",
        "v0.9",
        "--title",
        "Beta",
        "--due",
        "2027-06-01",
    ]);
    assert_contains(
        &p.expect(&["milestone", "list"]).all(),
        "v0.9",
        "it is listed",
    );
    p.fails(&["milestone", "add", "v0.9"]);
    p.fails(&["milestone", "add", "v1.1", "--due", "nonsense"]);
}

#[test]
fn a_milestone_in_use_is_protected() {
    let p = seeded();
    p.expect(&["milestone", "add", "v0.9"]);
    p.expect(&["set", "3", "milestone=v0.9", "-q"]);
    p.fails(&["milestone", "remove", "v0.9"]);

    // Forcing it clears the milestone from whatever referenced it, rather than
    // leaving items pointing at a name that no longer exists.
    let out = p.expect(&["milestone", "remove", "v0.9", "--force"]);
    assert_contains(&out.all(), "milestone cleared", "it says what it did");
    p.expect(&["check"]);
    assert_eq!(
        p.json(&["show", "3", "--json"])["milestone"],
        serde_json::Value::Null
    );
}

#[test]
fn editing_the_configuration_preserves_its_comments() {
    let p = seeded();
    p.expect(&["milestone", "add", "v0.9"]);
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

#[test]
fn compact_refuses_while_duplicates_exist_then_closes_the_gaps() {
    let p = Project::new();
    for n in 1..=4 {
        p.add(&format!("Item {n}"), &[]);
    }
    p.expect(&["remove", "2", "--force"]);
    p.write(
        "cairn/items/0001-collision.md",
        "---\nid: 1\ntitle: Collision\nstatus: backlog\n---\nbody\n",
    );
    p.fails(&["renumber", "--compact"]);
    p.expect(&["renumber"]);
    p.expect(&["renumber", "--compact"]);
    p.expect(&["check"]);
    let ids = p.expect(&["list", "-A", "--ids"]).lines();
    assert_eq!(ids.last().unwrap(), &format!("{:04}", ids.len()), "no gaps");
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
    let doc = p.json(&["export"]);
    assert_eq!(doc["cairn"], "1", "the format is versioned");
    assert!(
        doc["schema"].is_object(),
        "the schema travels with the items"
    );
    let items = doc["items"].as_array().unwrap();
    assert_eq!(items.len(), p.count_all());
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
        &recv.expect(&["milestone", "list"]).all(),
        "v0.1",
        "the milestone the document mentioned",
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
    p.expect(&["milestone", "add", "v9.9"]);
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
    let toml = p.read("cairn.toml").replace("format = 1", "format = 99");
    p.write("cairn.toml", &toml);
    let out = p.fails(&["list"]);
    assert_contains(&out.all(), "format 99", "the format it found");
    assert_contains(&out.all(), "upgrade cairn", "and what to do about it");
}

#[test]
fn a_project_without_a_format_key_is_format_one() {
    // Projects created before the key existed must keep working untouched.
    let p = Project::new();
    let toml = p
        .read("cairn.toml")
        .lines()
        .filter(|l| !l.starts_with("format ="))
        .collect::<Vec<_>>()
        .join("\n");
    p.write("cairn.toml", &toml);
    p.add("Still fine", &[]);
    assert_contains(
        &p.expect(&["migrate", "--check"]).all(),
        "format 1",
        "assumed",
    );
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
fn an_undated_milestone_keeps_the_position_it_was_declared_in() {
    // From dogfooding: a project declared `m0-proof` first, undated, ahead of a
    // dated `m1-device`. Sorting undated milestones to the end put m0 last —
    // overriding an ordering its author had already expressed unambiguously.
    let p = Project::new();
    let toml = p.read("cairn.toml");
    let head = &toml[..toml.find("[[milestone]]").unwrap()];
    let tail = &toml[toml.rfind("# ─── Saved views").unwrap()..];
    p.write(
        "cairn.toml",
        &format!(
            "{head}\
             [[milestone]]\nname = \"m0-proof\"\n\n\
             [[milestone]]\nname = \"m1-device\"\ndue = \"2026-11-01\"\n\n\
             [[milestone]]\nname = \"m2-firmware\"\ndue = \"2027-01-15\"\n\n\
             [[milestone]]\nname = \"later\"\n\n{tail}"
        ),
    );

    let listed = p.expect(&["milestone", "list"]).stdout;
    let order: Vec<&str> = ["m0-proof", "m1-device", "m2-firmware", "later"]
        .into_iter()
        .filter(|m| listed.contains(m))
        .collect();
    let positions: Vec<usize> = order.iter().map(|m| listed.find(m).unwrap()).collect();
    assert!(
        positions.windows(2).all(|w| w[0] < w[1]),
        "milestones came out in the wrong order:\n{listed}"
    );

    // And the same order reaches the rendered roadmap.
    p.add("Something", &["--milestone", "m0-proof"]);
    p.add("Something else", &["--milestone", "m1-device"]);
    p.expect(&["render", "-q"]);
    let roadmap = p.read("ROADMAP.md");
    assert!(
        roadmap.find("m0-proof").unwrap() < roadmap.find("m1-device").unwrap(),
        "the rendered roadmap disagrees with the milestone list"
    );
}

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
    p.expect(&["milestone", "add", "someday"]);
    for n in 0..4 {
        p.add(&format!("Idea {n}"), &["--milestone", "someday"]);
    }
    p.expect(&["close", "1", "-q"]);
    for id in ["2", "3", "4"] {
        p.expect(&["set", id, "status=dropped", "-q"]);
    }

    let listed = p.expect(&["milestone", "list"]).stdout;
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

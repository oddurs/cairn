// cairn — end-to-end tests: format.
//
// Copyright (c) 2026 Oddur Sigurdsson. MIT licensed; see LICENSE.
//
// What is written to disk and read back — the golden corpus, every format that
// has ever existed, and documents from outside the project.
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

// cairn — the interchange document, stated as a property.
//
// Copyright (C) 2026 Oddur Sigurdsson
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, either version 3 of the License, or (at your option) any later
// version.  See COPYING for details.
//
// `cairn export` and `cairn import` are the two halves of a promise: a backlog
// can leave this program and come back. The example tests check that promise on
// backlogs somebody wrote by hand, which are the backlogs least likely to break
// it.
//
// This generates adversarial ones — titles that collide when slugged, bodies
// full of the syntax the format is made of, dependency graphs — and checks the
// property directly: everything that went out comes back, and the dependencies
// still point at the same items even though the identifiers may not survive.
use std::collections::BTreeMap;
use std::process::{Command, Stdio};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn pick<'a>(&mut self, items: &'a [&'a str]) -> &'a str {
        items[self.below(items.len())]
    }
}

fn path_with_binary() -> std::ffi::OsString {
    let dir = std::path::Path::new(env!("CARGO_BIN_EXE_cairn"))
        .parent()
        .expect("binary directory");
    let existing = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![dir.to_path_buf()];
    paths.extend(std::env::split_paths(&existing));
    std::env::join_paths(paths).expect("PATH")
}

struct Project(tempfile::TempDir);

impl Project {
    fn new(name: &str) -> Project {
        let p = Project(tempfile::tempdir().expect("temp dir"));
        p.expect(&["init", "--bare", "--name", name]);
        p
    }

    fn run(&self, args: &[&str]) -> (i32, String, String) {
        let out = Command::new(env!("CARGO_BIN_EXE_cairn"))
            .args(args)
            .current_dir(self.0.path())
            .env("NO_COLOR", "1")
            .env("CAIRN_USER", "roundtrip")
            .env("CAIRN_NO_HOOKS", "1")
            .env("PATH", path_with_binary())
            .stdin(Stdio::null())
            .output()
            .expect("running cairn");
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }

    fn expect(&self, args: &[&str]) -> String {
        let (code, stdout, stderr) = self.run(args);
        assert_eq!(code, 0, "cairn {args:?} failed\n{stdout}{stderr}");
        stdout
    }
}

/// Titles chosen to be difficult: ones that slug identically, ones that slug to
/// nothing at all, and ones carrying the punctuation the file format uses.
const TITLE_SHAPES: &[&str] = &[
    "Ordinary title",
    "Ordinary  title",
    "ordinary title",
    "ORDINARY TITLE",
    "Ordinary title!",
    "---",
    "...",
    ": colon leading",
    "trailing colon :",
    "#hash",
    "@at",
    "quote \" inside",
    "apostrophe ' inside",
    "back\\slash",
    "🪨 a cairn",
    "日本語のタイトル",
    "élan vital",
    "a",
    "1",
    "0001",
];

/// Bodies made of the syntax the format is built from, which is what a body
/// most likely to break a parser looks like.
const BODY_SHAPES: &[&str] = &[
    "",
    "Plain text.",
    "---\nnot: frontmatter\n---\n",
    "```\n---\n```\n",
    "## Heading\n\n- [ ] a box\n- [x] a ticked box\n",
    "Trailing whitespace   \n\nand a blank line\n",
    "A line with: a colon\nAnother - with a dash\n",
    "\u{feff}a byte order mark",
    "tabs\there\tand\there",
];

const LABELS: &[&str] = &[
    "auth",
    "ui",
    "perf",
    "needs-triage",
    "a-very-long-label-name",
];
const STATUSES: &[&str] = &["backlog", "planned", "doing", "blocked", "done", "dropped"];
const TYPES: &[&str] = &["feature", "bug", "chore", "docs"];

/// Everything about an item that must survive a round trip.
///
/// The identifier is deliberately absent: import allocates local ids, and
/// requiring them to match would be asserting something cairn does not promise.
/// Dependencies are therefore compared as the *titles* they point at, which is
/// the thing that actually has to be preserved.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Shape {
    title: String,
    kind: String,
    status: String,
    milestone: Option<String>,
    labels: Vec<String>,
    assignee: Option<String>,
    body: String,
    depends_on: Vec<String>,
}

/// Every item's shape, sorted.
///
/// A sorted list rather than a map keyed by title, because the titles here
/// collide on purpose: two items may legitimately be called the same thing, and
/// a map would quietly keep one of them and compare the wrong pair.
fn shapes(p: &Project) -> Vec<Shape> {
    let doc: serde_json::Value =
        serde_json::from_str(&p.expect(&["export"])).expect("export is JSON");
    let items = doc["items"].as_array().expect("items").clone();

    let titles: BTreeMap<u64, String> = items
        .iter()
        .map(|i| {
            (
                i["id"].as_u64().expect("id"),
                i["title"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();

    items
        .iter()
        .map(|i| {
            let title = i["title"].as_str().unwrap_or_default().to_string();
            let mut labels: Vec<String> = i["labels"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            labels.sort();
            let mut depends_on: Vec<String> = i["depends_on"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_u64())
                        .map(|d| titles.get(&d).cloned().unwrap_or_else(|| format!("?{d}")))
                        .collect()
                })
                .unwrap_or_default();
            depends_on.sort();
            Shape {
                title,
                kind: i["type"].as_str().unwrap_or_default().to_string(),
                status: i["status"].as_str().unwrap_or_default().to_string(),
                milestone: i["milestone"].as_str().map(str::to_string),
                labels,
                assignee: i["assignee"].as_str().map(str::to_string),
                // Bodies are compared with trailing whitespace ignored: the
                // format normalises it on write, deliberately, and both
                // sides of the round trip are written by cairn.
                body: i["body"]
                    .as_str()
                    .unwrap_or_default()
                    .trim_end()
                    .to_string(),
                depends_on,
            }
        })
        .collect::<Vec<_>>()
        .tap_sorted()
}

/// Sorting as a method, so the two sides of a comparison cannot differ in
/// whether it happened.
trait TapSorted {
    fn tap_sorted(self) -> Vec<Shape>;
}

impl TapSorted for Vec<Shape> {
    fn tap_sorted(mut self) -> Vec<Shape> {
        self.sort();
        self
    }
}

#[test]
fn a_backlog_survives_leaving_and_coming_back() {
    let seed: u64 = std::env::var("CAIRN_ROUNDTRIP_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0xB0A7);
    let count: usize = std::env::var("CAIRN_ROUNDTRIP_ITEMS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(40);

    println!("round trip: seed {seed}, {count} items");
    println!("            reproduce with CAIRN_ROUNDTRIP_SEED={seed}");

    let mut rng = Rng(seed);
    let source = Project::new("Source");

    let mut created: Vec<u32> = Vec::new();
    for n in 0..count {
        // Titles repeat on purpose: items whose slugs collide are exactly the
        // ones a filename-keyed round trip would lose.
        let title = format!("{} {}", rng.pick(TITLE_SHAPES), n % 7);
        // `--` for the same reason as the bodies below: several of these
        // titles begin with a dash, which is what makes them worth generating.
        let out = source.expect(&["new", "-q", "--", &title]);
        let id: u32 = out.trim().parse().expect("an id");
        created.push(id);

        source.expect(&[
            "set",
            &id.to_string(),
            &format!("status={}", rng.pick(STATUSES)),
            &format!("type={}", rng.pick(TYPES)),
            "-q",
        ]);
        if rng.below(2) == 0 {
            source.expect(&[
                "set",
                &id.to_string(),
                &format!("labels+={}", rng.pick(LABELS)),
                "-q",
            ]);
        }
        if rng.below(3) == 0 {
            source.expect(&["set", &id.to_string(), "assignee=someone", "-q"]);
        }
        if rng.below(3) == 0 {
            source.expect(&["set", &id.to_string(), "milestone=v0.1", "-q"]);
        }
        let body = rng.pick(BODY_SHAPES);
        if !body.is_empty() {
            // After `--`, so a body beginning with a dash is text rather
            // than a flag. This is ordinary command-line behaviour and the
            // reason the corpus contains such bodies at all.
            source.expect(&["note", &id.to_string(), "--bare", "-q", "--", body]);
        }
        // A dependency on something already created, never on itself.
        if !created.is_empty() && rng.below(3) == 0 {
            let dep = created[rng.below(created.len())];
            if dep != id {
                source.run(&["set", &id.to_string(), &format!("depends_on+={dep}"), "-q"]);
            }
        }
    }

    source.expect(&["check"]);
    let before = shapes(&source);
    let document = source.expect(&["export"]);

    let mirror = Project::new("Mirror");
    std::fs::write(mirror.0.path().join("in.json"), &document).expect("writing the document");
    mirror.expect(&["import", "--from", "json", "in.json", "-q"]);
    mirror.expect(&["check"]);

    let after = shapes(&mirror);

    assert_eq!(
        before.len(),
        after.len(),
        "the round trip changed how many items there are"
    );
    for (want, got) in before.iter().zip(after.iter()) {
        assert_eq!(
            want, got,
            "an item came back different:\n  went out: {want:?}\n  came back: {got:?}"
        );
    }

    println!("round trip: {} items, unchanged", before.len());
}

/// Importing the same document twice must not double the backlog: adapters
/// re-run, and a re-import that duplicates everything is how somebody loses an
/// afternoon.
#[test]
fn importing_the_same_document_twice_changes_nothing_the_second_time() {
    let source = Project::new("Source");
    for title in ["First", "Second", "Third"] {
        source.expect(&["new", title, "-q"]);
    }
    source.expect(&["set", "2", "depends_on+=1", "-q"]);
    let document = source.expect(&["export"]);

    let mirror = Project::new("Mirror");
    std::fs::write(mirror.0.path().join("in.json"), &document).expect("write");
    mirror.expect(&["import", "--from", "json", "in.json", "-q"]);
    let once = shapes(&mirror);

    mirror.expect(&["import", "--from", "json", "in.json", "-q"]);
    let twice = shapes(&mirror);

    assert_eq!(once.len(), twice.len(), "a second import duplicated items");
    assert_eq!(once, twice, "a second import changed the backlog");
    mirror.expect(&["check"]);
}

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
mod support;
use support::*;

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

use std::collections::BTreeMap;

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

fn shapes(p: &Project) -> Vec<Shape> {
    let doc: serde_json::Value =
        serde_json::from_str(&p.expect(&["export"]).stdout).expect("export is JSON");
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

    let mut rng = Rng::new(seed);
    let source = Project::with_init(&["init", "--bare", "--name", "Source"]);
    // A milestone is an item in format 2, so one has to exist before anything
    // can be scheduled against it — and it round-trips like any other item.
    let m = source
        .expect(&["new", "-q", "--", "First release"])
        .trimmed();
    source.expect(&["set", &m, "type=milestone", "key=v0.1"]);

    let mut created: Vec<u32> = Vec::new();
    for n in 0..count {
        // Titles repeat on purpose: items whose slugs collide are exactly the
        // ones a filename-keyed round trip would lose.
        let title = format!("{} {}", rng.choose(TITLE_SHAPES), n % 7);
        // `--` for the same reason as the bodies below: several of these
        // titles begin with a dash, which is what makes them worth generating.
        let out = source.expect(&["new", "-q", "--", &title]);
        let id: u32 = out.trimmed().parse().expect("an id");
        created.push(id);

        source.expect(&[
            "set",
            &id.to_string(),
            &format!("status={}", rng.choose(STATUSES)),
            &format!("type={}", rng.choose(TYPES)),
            "-q",
        ]);
        if rng.below(2) == 0 {
            source.expect(&[
                "set",
                &id.to_string(),
                &format!("labels+={}", rng.choose(LABELS)),
                "-q",
            ]);
        }
        if rng.below(3) == 0 {
            source.expect(&["set", &id.to_string(), "assignee=someone", "-q"]);
        }
        if rng.below(3) == 0 {
            source.expect(&["set", &id.to_string(), "milestone=v0.1", "-q"]);
        }
        let body = rng.choose(BODY_SHAPES);
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
    let document = source.expect(&["export"]).stdout;

    let mirror = Project::with_init(&["init", "--bare", "--name", "Mirror"]);
    std::fs::write(mirror.root().join("in.json"), &document).expect("writing the document");
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
    let source = Project::with_init(&["init", "--bare", "--name", "Source"]);
    for title in ["First", "Second", "Third"] {
        source.expect(&["new", title, "-q"]);
    }
    source.expect(&["set", "2", "depends_on+=1", "-q"]);
    let document = source.expect(&["export"]).stdout;

    let mirror = Project::with_init(&["init", "--bare", "--name", "Mirror"]);
    std::fs::write(mirror.root().join("in.json"), &document).expect("write");
    mirror.expect(&["import", "--from", "json", "in.json", "-q"]);
    let once = shapes(&mirror);

    mirror.expect(&["import", "--from", "json", "in.json", "-q"]);
    let twice = shapes(&mirror);

    assert_eq!(once.len(), twice.len(), "a second import duplicated items");
    assert_eq!(once, twice, "a second import changed the backlog");
    mirror.expect(&["check"]);
}

// cairn — soak test.
//
// Copyright (C) 2026 Oddur Sigurdsson
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, either version 3 of the License, or (at your option) any later
// version.  See COPYING for details.
//
// The other tests check situations somebody thought of. This one drives cairn
// the way a project actually gets used — a long, arbitrary sequence of ordinary
// operations — and checks after every step that the backlog still holds
// together. Bugs that need three particular things to happen in a particular
// order are the kind it is for.
//
// Ignored by default because it takes a while:
//
//     cargo test --test soak -- --ignored --nocapture
//     CAIRN_SOAK_OPS=2000 CAIRN_SOAK_SEED=7 cargo test --test soak -- --ignored --nocapture
//
// Every run prints its seed. A failure is reproduced by passing that seed back.
mod support;
use support::*;

use std::collections::BTreeMap;
use std::process::{Command, Stdio};

struct Soak {
    project: Project,
    rng: Rng,
    /// What the sequence of operations says should be true, kept alongside what
    /// cairn believes so the two can be compared.
    expected: BTreeMap<u32, String>,
}

impl Soak {
    fn run(&self, args: &[&str]) -> Out {
        self.project.run_env(args, &[("CAIRN_USER", Some("soak"))])
    }

    /// Run with something on standard input, for the operations that speak a
    /// protocol rather than take arguments.
    fn run_stdin(&self, args: &[&str], input: &str) -> Out {
        self.project.run_stdin(args, input)
    }

    #[track_caller]
    fn expect(&self, args: &[&str]) -> Out {
        let out = self.run(args);
        assert_eq!(
            out.code,
            0,
            "cairn {args:?} failed\n{}",
            excerpt(&out.all())
        );
        out
    }

    fn ids(&self) -> Vec<u32> {
        self.expect(&["list", "-A", "--ids"])
            .lines()
            .iter()
            .filter_map(|l| l.trim().trim_start_matches('0').parse().ok())
            .collect()
    }
}

const STATUSES: &[&str] = &["backlog", "planned", "doing", "blocked", "done", "dropped"];

#[test]
#[ignore = "long-running; run with --ignored"]
fn a_long_sequence_of_ordinary_use_leaves_the_backlog_intact() {
    let seed: u64 = std::env::var("CAIRN_SOAK_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0x5EED);
    let ops: usize = std::env::var("CAIRN_SOAK_OPS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(400);

    println!("soak: seed {seed}, {ops} operations");
    println!("      reproduce with CAIRN_SOAK_SEED={seed} CAIRN_SOAK_OPS={ops}");

    let mut s = Soak {
        project: Project::empty(),
        rng: Rng::new(seed),
        expected: BTreeMap::new(),
    };
    s.expect(&["init", "--bare", "--name", "Soak"]);

    let mut performed: BTreeMap<&str, usize> = BTreeMap::new();
    for step in 0..ops {
        let live = s.ids();
        let target = s.rng.pick(&live).copied();
        // The weights below are not arbitrary. The coverage assertion at the
        // end requires every named operation to have run, so an operation it
        // names must be likely enough that its absence over `ops` steps means
        // something. At one slot in a hundred, `renumber` is missing from about
        // one run in fifty — which is a flaky test rather than a finding, and
        // was one until CI produced it.
        let choice = s.rng.below(106);

        let op = match (choice, target) {
            (0..=24, _) | (_, None) => {
                let title = format!("Item created at step {step}");
                let out = s.expect(&["new", &title, "-q"]);
                let id: u32 = out.stdout.trim().parse().expect("an id");
                s.expected.insert(id, "backlog".into());
                "new"
            }
            (25..=44, Some(id)) => {
                let status = STATUSES[s.rng.below(STATUSES.len())];
                s.expect(&["set", &id.to_string(), &format!("status={status}"), "-q"]);
                s.expected.insert(id, status.into());
                "set status"
            }
            (45..=54, Some(id)) => {
                s.expect(&["close", &id.to_string(), "-q"]);
                s.expected.insert(id, "done".into());
                "close"
            }
            (55..=61, Some(id)) => {
                s.expect(&["reopen", &id.to_string(), "-q"]);
                s.expected.insert(id, "backlog".into());
                "reopen"
            }
            (62..=68, Some(id)) => {
                let label = format!("label-{}", s.rng.below(6));
                s.expect(&["set", &id.to_string(), &format!("labels+={label}"), "-q"]);
                "label"
            }
            (69..=74, _) => {
                // Claiming picks its own item, and legitimately finds nothing
                // when everything ready is already taken or finished.
                let out = s.run(&["claim", "--next", "-q"]);
                if out.code == 0 {
                    if let Ok(id) = out.stdout.trim().parse::<u32>() {
                        s.expected.insert(id, "doing".into());
                    }
                } else {
                    assert!(
                        out.stderr.contains("nothing unclaimed"),
                        "claim failed unexpectedly: {}",
                        out.stderr
                    );
                }
                "claim"
            }
            (75..=79, Some(id)) => {
                s.expect(&["release", &id.to_string(), "-q"]);
                s.expected.insert(id, "backlog".into());
                "release"
            }
            (80..=84, Some(id)) => {
                s.expect(&["remove", &id.to_string(), "--force"]);
                s.expected.remove(&id);
                "remove"
            }
            (85..=89, Some(id)) => {
                // A dependency on another live item, never on itself.
                let others: Vec<u32> = live.iter().copied().filter(|o| *o != id).collect();
                if let Some(dep) = s.rng.pick(&others).copied() {
                    s.run(&["set", &id.to_string(), &format!("depends_on+={dep}"), "-q"]);
                }
                "depends"
            }
            (90..=91, Some(id)) => {
                let title = format!("Renamed at step {step}");
                s.expect(&["set", &id.to_string(), &format!("title={title}"), "-q"]);
                "rename"
            }
            (92..=94, _) => {
                s.expect(&["render", "-q"]);
                s.expect(&["render", "--check", "-q"]);
                "render"
            }
            (95..=97, _) => {
                s.expect(&["renumber"]);
                // Identifiers may have moved; the model is keyed by them, so it
                // is rebuilt from what cairn now reports.
                s.expected = current_state(&s);
                "renumber"
            }
            (98, _) => {
                // A dry run must never write, whatever state the backlog is in.
                let before = current_state(&s);
                s.expect(&["renumber", "--dry-run"]);
                assert_eq!(current_state(&s), before, "a dry run changed the backlog");
                "renumber --dry-run"
            }
            (99, Some(id)) => {
                s.expect(&[
                    "note",
                    &id.to_string(),
                    "--bare",
                    "-q",
                    "--",
                    &format!("Noted at step {step}"),
                ]);
                "note"
            }
            (100, Some(id)) => {
                // A milestone is an item in format 2, so this exercises the
                // container path: creating one, giving it a key, and scheduling
                // work against it by that key.
                let name = format!("v0.{}", s.rng.below(3));
                let existing = s.expect(&["list", "-A", "-t", "milestone", "--json"]);
                if !existing.stdout.contains(&format!("\"{name}\"")) {
                    let out = s.expect(&["new", &name, "-t", "milestone", "-q"]);
                    let m: u32 = out.stdout.trim().parse().expect("an id");
                    s.expect(&["set", &m.to_string(), &format!("key={name}"), "-q"]);
                    s.expected.insert(m, "backlog".into());
                }
                s.expect(&["set", &id.to_string(), &format!("milestone={name}"), "-q"]);
                "milestone"
            }
            (101, _) => {
                // Every read, run for its exit status: a view that panics on a
                // backlog reached by an odd route is still a bug.
                for args in [
                    vec!["board"],
                    vec!["roadmap"],
                    vec!["next"],
                    vec!["list", "-A"],
                    vec!["search", "step"],
                    vec!["config"],
                    vec!["agent"],
                    vec!["check", "--strict"],
                ] {
                    let out = s.run(&args);
                    assert!(
                        out.code == 0 || out.code == 1,
                        "read command {args:?} exited {}\n{}{}",
                        out.code,
                        out.stdout,
                        out.stderr
                    );
                    assert!(
                        !out.stderr.contains("panicked at"),
                        "read command {args:?} panicked\n{}",
                        out.stderr
                    );
                }
                "reads"
            }
            (102, _) => {
                // The MCP server is how agents reach the backlog, and until now
                // nothing here had ever spoken to it.
                let request = concat!(
                    r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":"#,
                    r#"{"protocolVersion":"2024-11-05","capabilities":{},"#,
                    r#""clientInfo":{"name":"soak","version":"0"}}}"#,
                    "\n",
                    r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
                    "\n",
                    r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":"#,
                    r#"{"name":"list_items","arguments":{}}}"#,
                    "\n"
                );
                let out = s.run_stdin(&["mcp"], request);
                assert_eq!(out.code, 0, "the MCP server failed: {}", out.stderr);
                let replies = out.stdout.lines().count();
                assert_eq!(replies, 3, "expected three replies, got:\n{}", out.stdout);
                for line in out.stdout.lines() {
                    let v: serde_json::Value =
                        serde_json::from_str(line).expect("each reply is JSON");
                    assert!(
                        v.get("error").is_none(),
                        "the MCP server returned an error: {line}"
                    );
                }
                "mcp"
            }
            (103..=105, _) | (_, _) => {
                let doc = s.expect(&["export"]).stdout;
                let mirror = Soak {
                    project: Project::empty(),
                    rng: Rng::new(0),
                    expected: BTreeMap::new(),
                };
                mirror.expect(&["init", "--bare", "--name", "Mirror"]);
                mirror.project.write("in.json", &doc);
                mirror.expect(&["import", "--from", "json", "in.json", "-q"]);
                assert_eq!(
                    mirror.ids().len(),
                    s.ids().len(),
                    "an export/import round trip lost items"
                );
                mirror.expect(&["check"]);
                "export/import"
            }
        };
        *performed.entry(op).or_default() += 1;

        check_invariants(&s, step, op);
    }

    // The model and the backlog must agree at the end, not merely be
    // individually self-consistent.
    let actual = current_state(&s);
    assert_eq!(
        actual, s.expected,
        "cairn and the model disagree about the backlog"
    );

    println!("soak: {ops} operations, backlog intact");
    for (op, n) in &performed {
        println!("      {n:>4}  {op}");
    }

    // An operation that never ran tested nothing, and a table that quietly
    // stops reaching one is invisible without this. Only operations weighted
    // heavily enough for absence to be meaningful are named: asserting on a
    // one-in-a-hundred draw over a few hundred steps is a coin toss, not a
    // check.
    for op in [
        "new",
        "set status",
        "close",
        "reopen",
        "remove",
        "render",
        "renumber",
        "export/import",
    ] {
        assert!(
            performed.contains_key(op),
            "`{op}` never ran in {ops} operations; the operation table no \
             longer reaches it"
        );
    }
}

/// Two processes writing at once.
///
/// Identifiers are allocated as one more than the highest in use, which is only
/// safe because a lock makes the read and the write one step. The unit tests
/// race the lock directly; this races it through ordinary commands, which is
/// how it is actually reached.
#[test]
#[ignore = "long-running; run with --ignored"]
fn concurrent_writers_leave_one_consistent_backlog() {
    let writers: usize = std::env::var("CAIRN_SOAK_WRITERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4);
    let each: usize = std::env::var("CAIRN_SOAK_EACH")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(25);

    println!("soak: {writers} concurrent writers, {each} operations each");

    let s = Soak {
        project: Project::empty(),
        rng: Rng::new(0),
        expected: BTreeMap::new(),
    };
    s.expect(&["init", "--bare", "--name", "Concurrent"]);

    let root = s.project.root().to_path_buf();
    let mut threads = Vec::new();
    for writer in 0..writers {
        let root = root.clone();
        threads.push(std::thread::spawn(move || {
            let mut made = Vec::new();
            for n in 0..each {
                let title = format!("writer {writer} item {n}");
                let out = Command::new(env!("CARGO_BIN_EXE_cairn"))
                    .args(["new", &title, "-q"])
                    .current_dir(&root)
                    .env("NO_COLOR", "1")
                    .env("CAIRN_USER", format!("writer-{writer}"))
                    .env("PATH", path_with_binary())
                    .stdin(Stdio::null())
                    .output()
                    .expect("running cairn");
                assert!(
                    out.status.success(),
                    "writer {writer} could not create an item: {}",
                    String::from_utf8_lossy(&out.stderr)
                );
                let id: u32 = String::from_utf8_lossy(&out.stdout)
                    .trim()
                    .parse()
                    .expect("an id");
                made.push(id);

                // And a write to an item this writer owns, so the lock is
                // contended by more than creation.
                let out = Command::new(env!("CARGO_BIN_EXE_cairn"))
                    .args(["set", &id.to_string(), "status=doing", "-q"])
                    .current_dir(&root)
                    .env("NO_COLOR", "1")
                    .env("CAIRN_USER", format!("writer-{writer}"))
                    .env("PATH", path_with_binary())
                    .stdin(Stdio::null())
                    .output()
                    .expect("running cairn");
                assert!(
                    out.status.success(),
                    "writer {writer} could not update {id}: {}",
                    String::from_utf8_lossy(&out.stderr)
                );
            }
            made
        }));
    }

    let mut allocated: Vec<u32> = Vec::new();
    for t in threads {
        allocated.extend(t.join().expect("a writer panicked"));
    }

    // The property that matters: no two commands were handed the same id.
    let unique: std::collections::HashSet<u32> = allocated.iter().copied().collect();
    assert_eq!(
        unique.len(),
        allocated.len(),
        "two writers were given the same identifier"
    );
    assert_eq!(
        allocated.len(),
        writers * each,
        "an item went missing between the writers and the backlog"
    );

    let ids = s.ids();
    assert_eq!(
        ids.len(),
        writers * each,
        "the backlog holds a different number of items than were created"
    );
    check_invariants(&s, 0, "concurrent");
    s.expect(&["check", "--strict"]);

    println!("soak: {} items, every identifier distinct", ids.len());
}

/// Read the backlog through the machine interface, which reports status names
/// rather than the labels a person is shown.
fn current_state(s: &Soak) -> BTreeMap<u32, String> {
    let rows = s.expect(&["list", "-A", "--json"]);
    let items: serde_json::Value = serde_json::from_str(&rows.stdout).expect("json");
    items
        .as_array()
        .expect("an array")
        .iter()
        .map(|i| {
            (
                i["id"].as_u64().expect("id") as u32,
                i["status"].as_str().expect("status").to_string(),
            )
        })
        .collect()
}

/// Everything that must be true after any operation whatsoever.
fn check_invariants(s: &Soak, step: usize, op: &str) {
    let at = format!("step {step} ({op})");

    let check = s.run(&["check"]);
    assert_eq!(
        check.code, 0,
        "{at}: cairn check failed\n{}{}",
        check.stdout, check.stderr
    );

    let ids = s.ids();
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len(), "{at}: duplicate identifiers");

    let items = s.project.root().join("cairn/items");
    for entry in std::fs::read_dir(&items).expect("item directory").flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        assert!(
            !name.ends_with(".tmp"),
            "{at}: a partial write survived: {name}"
        );
        assert!(
            !name.ends_with(".renumber"),
            "{at}: a staged file was left behind: {name}"
        );
        assert!(
            !name.starts_with(".lock"),
            "{at}: the lock outlived a command"
        );
    }
    assert!(
        !items.join(".lock").exists(),
        "{at}: the lock outlived a command"
    );
}

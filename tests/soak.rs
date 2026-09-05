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
use std::collections::BTreeMap;
use std::process::{Command, Stdio};

/// splitmix64. A dependency-free generator whose only requirement is that the
/// same seed replays the same run, so a failure can be reproduced exactly.
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
    fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            None
        } else {
            let i = self.below(items.len());
            items.get(i)
        }
    }
}

/// The default hooks invoke `cairn` by name, as an installed one would be.
fn path_with_binary() -> std::ffi::OsString {
    let dir = std::path::Path::new(env!("CARGO_BIN_EXE_cairn"))
        .parent()
        .expect("binary directory");
    let existing = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![dir.to_path_buf()];
    paths.extend(std::env::split_paths(&existing));
    std::env::join_paths(paths).expect("PATH")
}

struct Soak {
    dir: tempfile::TempDir,
    rng: Rng,
    /// What the sequence of operations says should be true, kept alongside what
    /// cairn believes so the two can be compared.
    expected: BTreeMap<u32, String>,
}

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Soak {
    fn run(&self, args: &[&str]) -> Out {
        let out = Command::new(env!("CARGO_BIN_EXE_cairn"))
            .args(args)
            .current_dir(self.dir.path())
            .env("NO_COLOR", "1")
            .env("CAIRN_USER", "soak")
            .env("PATH", path_with_binary())
            .stdin(Stdio::null())
            .output()
            .expect("running cairn");
        Out {
            code: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        }
    }

    fn expect(&self, args: &[&str]) -> Out {
        let out = self.run(args);
        assert_eq!(
            out.code, 0,
            "cairn {args:?} failed\nstdout: {}\nstderr: {}",
            out.stdout, out.stderr
        );
        out
    }

    fn ids(&self) -> Vec<u32> {
        self.expect(&["list", "-A", "--ids"])
            .stdout
            .lines()
            .filter_map(|l| l.trim().parse().ok())
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
        dir: tempfile::tempdir().expect("temp dir"),
        rng: Rng(seed),
        expected: BTreeMap::new(),
    };
    s.expect(&["init", "--bare", "--name", "Soak"]);

    let mut performed: BTreeMap<&str, usize> = BTreeMap::new();
    for step in 0..ops {
        let live = s.ids();
        let target = s.rng.pick(&live).copied();
        let choice = s.rng.below(100);

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
            (90..=93, Some(id)) => {
                let title = format!("Renamed at step {step}");
                s.expect(&["set", &id.to_string(), &format!("title={title}"), "-q"]);
                "rename"
            }
            (94..=96, _) => {
                s.expect(&["render", "-q"]);
                s.expect(&["render", "--check", "-q"]);
                "render"
            }
            (97..=98, _) => {
                s.expect(&["renumber"]);
                // Identifiers may have moved; the model is keyed by them, so it
                // is rebuilt from what cairn now reports.
                s.expected = current_state(&s);
                "renumber"
            }
            (_, _) => {
                let doc = s.expect(&["export"]).stdout;
                let round = tempfile::tempdir().unwrap();
                let mirror = Soak {
                    dir: round,
                    rng: Rng(0),
                    expected: BTreeMap::new(),
                };
                mirror.expect(&["init", "--bare", "--name", "Mirror"]);
                std::fs::write(mirror.dir.path().join("in.json"), &doc).unwrap();
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

    let items = s.dir.path().join("cairn/items");
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

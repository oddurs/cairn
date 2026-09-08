// cairn — the command line, fed arbitrary input.
//
// Copyright (C) 2026 Oddur Sigurdsson
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, either version 3 of the License, or (at your option) any later
// version.  See COPYING for details.
//
// Every argument surface takes text from a person or from a model, and a model
// gets them wrong in ways a person would not think to try. This drives the real
// binary with arbitrary argument vectors and asserts one thing: cairn may refuse
// anything, but it may not panic.
//
// The point is not the specific sequences. It is that they were not chosen by
// somebody who already knew where the bugs were — which is where this project's
// defects have actually come from.
mod support;
use support::*;

use std::collections::BTreeSet;
use std::process::{Command, Stdio};

/// Every subcommand, including the hidden ones: hidden from `--help` is not the
/// same as unreachable, and an unreachable command could not be fuzzed at all.
const COMMANDS: &[&str] = &[
    "init",
    "new",
    "add",
    "list",
    "ls",
    "next",
    "search",
    "grep",
    "claim",
    "release",
    "show",
    "log",
    "set",
    "note",
    "close",
    "reopen",
    "edit",
    "remove",
    "rm",
    "board",
    "roadmap",
    "render",
    "export",
    "import",
    "check",
    "renumber",
    "migrate",
    "merge-driver",
    "milestone",
    "config",
    "agent",
    "mcp",
    "completions",
    "man",
];

/// Flags drawn from across the tool, so they land on commands that do not take
/// them as often as on commands that do.
const FLAGS: &[&str] = &[
    "--json",
    "--plain",
    "--ids",
    "--count",
    "-q",
    "--quiet",
    "-A",
    "--all",
    "--force",
    "--yes",
    "-y",
    "--strict",
    "--check",
    "--render",
    "--no-hooks",
    "--bare",
    "--git",
    "--patch",
    "-p",
    "--filter",
    "--status",
    "--type",
    "--label",
    "--view",
    "--sort",
    "--columns",
    "--from",
    "--name",
    "--write",
    "--dir",
    "-n",
    "-C",
    "--color",
    "--stdin",
    "--heading",
    "-d",
    "-m",
    "-t",
    "--set",
    "--next",
    "--blocked",
    "--bug-report",
    "--version",
    "--help",
    "-h",
];

/// Values a model or a fat-fingered person actually produces.
const VALUES: &[&str] = &[
    "",
    "1",
    "0",
    "-1",
    "99999999999999999999",
    "4294967296",
    "0001",
    "1 2 3",
    "status=doing",
    "status=",
    "=",
    "==",
    "labels+=",
    "labels+=a",
    "depends_on+=1",
    "depends_on+=1,2",
    "title=",
    "priority=p0",
    "nonsense=value",
    "status=nonexistent",
    "--",
    "-",
    "---",
    "a,b,c",
    "a|b",
    "a~b",
    "!",
    "*",
    "?",
    "..",
    "../..",
    "/",
    ".",
    "auto",
    "always",
    "never",
    "json",
    "github",
    "bash",
    "élan",
    "日本語",
    "🪨",
    "\u{200b}",
    "line\nbreak",
    "tab\there",
    "quote\"inside",
    "back\\slash",
    "a very long value that goes on and on and on and is well past anything a person would type by hand and keeps going",
];

fn corpus(rng: &mut Rng) -> String {
    match rng.below(10) {
        0..=3 => rng.choose(COMMANDS).to_string(),
        4..=6 => rng.choose(FLAGS).to_string(),
        _ => rng.choose(VALUES).to_string(),
    }
}

/// A project to fuzz inside, with enough in it that commands reach past their
/// argument parsing. An empty directory would leave most of the tool untouched.
fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_cairn"))
            .args(args)
            .current_dir(dir.path())
            .env("NO_COLOR", "1")
            .env("CAIRN_USER", "fuzz")
            .env("CAIRN_NO_HOOKS", "1")
            .stdin(Stdio::null())
            .output()
            .expect("running cairn");
    };
    run(&["init", "--bare", "--name", "Fuzz"]);
    run(&["new", "First"]);
    run(&["new", "Second"]);
    run(&["set", "2", "depends_on+=1"]);
    dir
}

#[test]
fn arbitrary_arguments_never_panic() {
    let seed: u64 = std::env::var("CAIRN_FUZZ_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0xC0FFEE);
    let rounds: usize = std::env::var("CAIRN_FUZZ_ROUNDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(600);

    println!("fuzz: seed {seed}, {rounds} invocations");
    println!("      reproduce with CAIRN_FUZZ_SEED={seed} CAIRN_FUZZ_ROUNDS={rounds}");

    let dir = project();
    let mut rng = Rng::new(seed);
    let mut codes: BTreeSet<i32> = BTreeSet::new();

    for round in 0..rounds {
        let n = 1 + rng.below(5);
        let args: Vec<String> = (0..n).map(|_| corpus(&mut rng)).collect();

        let out = Command::new(env!("CARGO_BIN_EXE_cairn"))
            .args(&args)
            .current_dir(dir.path())
            .env("NO_COLOR", "1")
            .env("CAIRN_USER", "fuzz")
            .env("CAIRN_NO_HOOKS", "1")
            // `edit` would otherwise open something and wait forever. Naming an
            // editor that does not exist makes it fail fast, which is the
            // behaviour under test anyway.
            .env("EDITOR", "cairn-fuzz-has-no-editor")
            .env_remove("VISUAL")
            // Anything that reads standard input — `note --stdin`, `mcp`, a
            // confirmation — gets an immediate end of file rather than hanging.
            .stdin(Stdio::null())
            .output()
            .unwrap_or_else(|e| panic!("round {round}: running cairn {args:?}: {e}"));

        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            !stderr.contains("panicked at"),
            "round {round}: cairn {args:?} panicked\n{stderr}"
        );

        let code = out.status.code().unwrap_or(-1);
        assert_ne!(
            code, 101,
            "round {round}: cairn {args:?} exited with a panic status\n{stderr}"
        );
        assert!(
            (0..=2).contains(&code),
            "round {round}: cairn {args:?} exited {code}, which is not one of \
             the documented statuses\n{stderr}"
        );
        codes.insert(code);
    }

    println!("fuzz: {rounds} invocations, exit statuses seen: {codes:?}");
    // If every invocation succeeded, the corpus is not reaching the parser and
    // this test is asserting nothing.
    assert!(
        codes.len() > 1,
        "every invocation returned the same status; the fuzzer is not \
         exercising anything"
    );
}

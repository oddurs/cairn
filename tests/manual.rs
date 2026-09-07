// cairn — every command the manual shows, run against the real program.
//
// Copyright (C) 2026 Oddur Sigurdsson
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, either version 3 of the License, or (at your option) any later
// version.  See COPYING for details.
//
// Documentation is the least-tested surface in most projects, and it was in this
// one: a chapter shipped claiming `cairn export --json`, `cairn check --json`
// and a filter written `status:open priority:p0`. None of the three existed.
// Prose is easy to write and nothing disagrees with it.
//
// So every `cairn ...` line in an @example block is extracted and run. The test
// is not that each succeeds — several are meant to fail, and one imports from a
// service that is not there — but that none is rejected as a *malformed command
// line*. Exit status 2 means the arguments were wrong, and an example with wrong
// arguments is an example nobody can follow.
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn manual() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("doc/cairn.texi");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
        .replace("\r\n", "\n")
}

/// Every `cairn ...` command inside an `@example` block.
///
/// Lines are taken whole and split like a shell would, minus the parts a shell
/// does and cairn cannot be asked about: a redirection, a pipe, or a `$` prompt.
fn commands(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("@example") {
            inside = true;
            continue;
        }
        if trimmed.starts_with("@end example") {
            inside = false;
            continue;
        }
        if !inside {
            continue;
        }
        let line = trimmed.trim_start_matches("$ ").trim();
        let Some(rest) = line.strip_prefix("cairn ") else {
            continue;
        };
        // A comment, a pipe or a redirection is the shell's business.
        let rest = rest.split('#').next().unwrap_or(rest);
        let rest = rest.split('|').next().unwrap_or(rest);
        // Both redirections: `< file` was missed the first time, and the test
        // duly reported a manual command as broken when the manual was right.
        let rest = rest.split('>').next().unwrap_or(rest);
        let rest = rest.split('<').next().unwrap_or(rest);
        // Texinfo escapes braces, which the reader of the manual does not type.
        let rest = rest.replace("@{", "{").replace("@}", "}");
        if !rest.trim().is_empty() {
            out.push(rest.trim().to_string());
        }
    }
    out
}

/// Split on spaces, keeping single-quoted arguments together — the filter
/// expressions in the manual are quoted, and splitting them would test
/// something the reader never types.
fn argv(command: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for c in command.chars() {
        match (quote, c) {
            (Some(q), _) if c == q => quote = None,
            (Some(_), _) => current.push(c),
            (None, '\'') | (None, '"') => quote = Some(c),
            (None, ' ') => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            (None, _) => current.push(c),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

#[test]
fn every_command_in_the_manual_is_one_cairn_accepts() {
    let text = manual();
    let commands = commands(&text);
    assert!(
        commands.len() > 40,
        "only {} commands found; the extractor has stopped matching the manual",
        commands.len()
    );

    // A project with enough in it that commands reach past argument parsing.
    let dir = tempfile::tempdir().expect("temp dir");
    let run = |args: &[String], cwd: &std::path::Path| {
        Command::new(env!("CARGO_BIN_EXE_cairn"))
            .args(args)
            .current_dir(cwd)
            .env("NO_COLOR", "1")
            .env("CAIRN_USER", "manual")
            .env("CAIRN_NO_HOOKS", "1")
            .env("EDITOR", "cairn-manual-has-no-editor")
            .env_remove("VISUAL")
            .stdin(Stdio::null())
            .output()
            .expect("running cairn")
    };
    for setup in [
        vec!["init", "--bare", "--name", "Manual"],
        vec!["new", "First"],
        vec!["new", "Second"],
        vec!["milestone", "add", "v0.1"],
        vec!["milestone", "add", "v0.2"],
    ] {
        let args: Vec<String> = setup.iter().map(|s| s.to_string()).collect();
        run(&args, dir.path());
    }

    let mut rejected = Vec::new();
    let mut seen = BTreeSet::new();
    for command in &commands {
        if !seen.insert(command.clone()) {
            continue;
        }
        let args = argv(command);
        // `init` would create a second project inside this one, and `mcp` waits
        // on a protocol. Neither is about argument parsing.
        if matches!(args.first().map(String::as_str), Some("init") | Some("mcp")) {
            continue;
        }

        let out = run(&args, dir.path());
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            !stderr.contains("panicked at"),
            "`cairn {command}` panicked:\n{stderr}"
        );
        // 2 is "the command line was wrong", and is the only status that means
        // the example itself is broken. See the Stability chapter.
        if out.status.code() == Some(2) {
            rejected.push(format!("  cairn {command}\n    {}", stderr.trim()));
        }
    }

    assert!(
        rejected.is_empty(),
        "the manual shows {} command(s) cairn would reject:\n{}",
        rejected.len(),
        rejected.join("\n")
    );
}

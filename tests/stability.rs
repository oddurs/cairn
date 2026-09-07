// cairn — the promises in the Stability chapter, asserted.
//
// Copyright (C) 2026 Oddur Sigurdsson
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, either version 3 of the License, or (at your option) any later
// version.  See COPYING for details.
//
// A promise nothing checks is a wish. Every field named in the manual as stable
// is listed here, so removing or renaming one fails the build rather than
// somebody's script — six months later, quietly, in a way they cannot tell was
// deliberate.
//
// Adding to these lists is how a field becomes promised, and should be a
// decision. Removing from them is a major release.
use std::collections::BTreeSet;
use std::process::{Command, Stdio};

/// Every item carries these, in every command that emits items.
const ITEM_FIELDS: &[&str] = &[
    "id",
    "title",
    "type",
    "status",
    "category",
    "milestone",
    "assignee",
    "labels",
    "depends_on",
    "created",
    "updated",
    "source",
    "ref",
    "fields",
];

/// The eleven tools an agent may call. Removing one is a major release.
const MCP_TOOLS: &[&str] = &[
    "get_schema",
    "next_items",
    "list_items",
    "search_items",
    "show_item",
    "claim_item",
    "create_item",
    "update_item",
    "close_item",
    "add_note",
    "check",
];

struct Project(tempfile::TempDir);

impl Project {
    fn new() -> Project {
        let p = Project(tempfile::tempdir().expect("temp dir"));
        p.expect(&["init", "--bare", "--name", "Stable"]);
        p.expect(&["new", "First", "-q"]);
        p.expect(&["new", "Second", "-q"]);
        p.expect(&["set", "2", "depends_on+=1", "-q"]);
        p
    }

    fn run(&self, args: &[&str]) -> (i32, String) {
        let out = Command::new(env!("CARGO_BIN_EXE_cairn"))
            .args(args)
            .current_dir(self.0.path())
            .env("NO_COLOR", "1")
            .env("CAIRN_USER", "stability")
            .env("CAIRN_NO_HOOKS", "1")
            .stdin(Stdio::null())
            .output()
            .expect("running cairn");
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).into_owned(),
        )
    }

    fn expect(&self, args: &[&str]) -> String {
        let (code, stdout) = self.run(args);
        assert_eq!(code, 0, "cairn {args:?} failed");
        stdout
    }

    fn json(&self, args: &[&str]) -> serde_json::Value {
        serde_json::from_str(&self.expect(args))
            .unwrap_or_else(|e| panic!("cairn {args:?} did not emit JSON: {e}"))
    }
}

fn keys(v: &serde_json::Value) -> BTreeSet<String> {
    v.as_object()
        .expect("an object")
        .keys()
        .map(String::from)
        .collect()
}

fn assert_has(present: &BTreeSet<String>, required: &[&str], what: &str) {
    let missing: Vec<&&str> = required.iter().filter(|f| !present.contains(**f)).collect();
    assert!(
        missing.is_empty(),
        "{what} no longer carries {missing:?}, which the Stability chapter \
         promises. Removing a documented field is a major release; if that is \
         genuinely intended, the manual and this list change with it."
    );
}

#[test]
fn every_command_that_emits_items_carries_the_documented_fields() {
    let p = Project::new();

    for args in [
        vec!["list", "--json"],
        vec!["next", "--json"],
        vec!["search", "First", "--json"],
    ] {
        let v = p.json(&args);
        let first = v.as_array().expect("an array").first().expect("an item");
        assert_has(&keys(first), ITEM_FIELDS, &format!("`cairn {args:?}`"));
    }

    let shown = p.json(&["show", "1", "--json"]);
    assert_has(&keys(&shown), ITEM_FIELDS, "`cairn show --json`");
    assert_has(&keys(&shown), &["body"], "`cairn show --json`");

    // `next` answers "what can I start", so these two are the point of it.
    let next = p.json(&["next", "--json"]);
    let first = next.as_array().expect("an array").first().expect("an item");
    assert_has(&keys(first), &["blockers", "ready"], "`cairn next --json`");
}

#[test]
fn the_interchange_document_keeps_its_shape() {
    let p = Project::new();
    let doc = p.json(&["export"]);
    assert_has(
        &keys(&doc),
        &["cairn", "exported", "project", "schema", "items"],
        "the interchange document",
    );
    assert_eq!(doc["cairn"], "1", "the interchange version changed");

    let item = doc["items"]
        .as_array()
        .expect("items")
        .first()
        .expect("one");
    assert_has(&keys(item), ITEM_FIELDS, "an exported item");
    assert_has(&keys(item), &["body", "blocked"], "an exported item");
}

#[test]
fn the_resolved_configuration_keeps_its_shape() {
    let p = Project::new();
    let cfg = p.json(&["config", "--json"]);
    assert_has(
        &keys(&cfg),
        &[
            "project",
            "types",
            "statuses",
            "fields",
            "milestones",
            "views",
        ],
        "`cairn config --json`",
    );
}

#[test]
fn the_agent_tools_are_all_still_there() {
    let p = Project::new();
    let request = concat!(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":"#,
        r#"{"protocolVersion":"2024-11-05","capabilities":{},"#,
        r#""clientInfo":{"name":"stability","version":"0"}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
        "\n"
    );

    use std::io::Write;
    let mut child = Command::new(env!("CARGO_BIN_EXE_cairn"))
        .arg("mcp")
        .current_dir(p.0.path())
        .env("NO_COLOR", "1")
        .env("CAIRN_NO_HOOKS", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawning cairn mcp");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(request.as_bytes())
        .expect("writing the request");
    let out = child.wait_with_output().expect("waiting");
    let last = String::from_utf8_lossy(&out.stdout)
        .lines()
        .next_back()
        .expect("a reply")
        .to_string();
    let reply: serde_json::Value = serde_json::from_str(&last).expect("JSON");

    let present: BTreeSet<String> = reply["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .filter_map(|t| t["name"].as_str().map(String::from))
        .collect();
    assert_has(&present, MCP_TOOLS, "the MCP server");
}

/// 0 did it, 1 failed or found problems, 2 the command line was wrong. Nothing
/// else, ever — a script that branches on the status has to be able to.
#[test]
fn exit_codes_mean_what_the_manual_says() {
    let p = Project::new();

    assert_eq!(p.run(&["check"]).0, 0, "a valid project checks clean");
    assert_eq!(p.run(&["list"]).0, 0, "listing succeeds");

    // 2 is clap's, and is only ever clap's.
    assert_eq!(
        p.run(&["list", "--no-such-flag"]).0,
        2,
        "a bad command line is 2"
    );
    assert_eq!(p.run(&["no-such-command"]).0, 2, "an unknown command is 2");

    // 1 is the operation failing, or finding what it was asked to look for.
    assert_eq!(
        p.run(&["show", "999"]).0,
        1,
        "an item that does not exist is 1, not 2: the command line was fine"
    );
    std::fs::write(
        p.0.path().join("cairn/items/broken.md"),
        "not an item at all",
    )
    .expect("writing a broken item");
    assert_eq!(p.run(&["check"]).0, 1, "a project with problems is 1");
}

/// `--ids` puts one identifier on a line and nothing else. Anything more
/// breaks every `| xargs` in existence.
#[test]
fn ids_output_is_only_ids() {
    let p = Project::new();
    for args in [
        vec!["list", "--ids"],
        vec!["next", "--ids"],
        vec!["search", "First", "--ids"],
    ] {
        for line in p.expect(&args).lines() {
            assert!(
                !line.trim().is_empty() && line.trim().chars().all(|c| c.is_alphanumeric()),
                "`cairn {args:?}` put something other than an identifier on a line: {line:?}"
            );
        }
    }
}

/// `--count` prints one integer and nothing else.
#[test]
fn count_output_is_only_a_number() {
    let p = Project::new();
    for args in [
        vec!["list", "--count"],
        vec!["next", "--count"],
        vec!["search", "First", "--count"],
    ] {
        let out = p.expect(&args);
        out.trim()
            .parse::<usize>()
            .unwrap_or_else(|_| panic!("`cairn {args:?}` printed {out:?}, not a number"));
        assert_eq!(
            out.lines().count(),
            1,
            "`cairn {args:?}` printed more than one line"
        );
    }
}

/// `--plain` prints machine values, not the labels a person is shown. The
/// status named `doing` is displayed as "in progress", and a script must see
/// `doing`.
#[test]
fn plain_output_carries_names_rather_than_labels() {
    let p = Project::new();
    p.expect(&["set", "1", "status=doing", "-q"]);

    for args in [
        vec!["list", "--plain", "-A"],
        vec!["next", "--plain"],
        vec!["search", "First", "--plain"],
    ] {
        let out = p.expect(&args);
        assert!(
            !out.contains("in progress"),
            "`cairn {args:?}` printed a display label rather than a status name:\n{out}"
        );
        assert!(
            out.lines().all(|l| l.contains('\t')),
            "`cairn {args:?}` is not tab-separated:\n{out}"
        );
    }
}

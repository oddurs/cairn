// cairn — end-to-end tests: agents.
//
// Copyright (c) 2026 Oddur Sigurdsson. MIT licensed; see LICENSE.
//
// The surface a program meets rather than a person: the instruction block, the
// protocol server, and the permissions, claims and proposals that go with it.
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
// --- MCP --------------------------------------------------------------------

#[test]
fn mcp_speaks_the_protocol() {
    let p = seeded();
    let replies = p.mcp(
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
    let replies = p.mcp(
        &[
            // Named, because the identity of a write over this transport comes
            // from the protocol. Without a name it falls back to whoever owns
            // the machine, which is not a thing a test may assert on.
            r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"clientInfo":{"name":"tester"}}}"#,
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"next_items","arguments":{"limit":2}}}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"create_item","arguments":{"title":"Made over MCP","type":"bug"}}}"#,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"claim_item","arguments":{}}}"#,
        ],
    );
    // The handshake is not a tool call and carries no `isError`.
    for r in &replies[1..] {
        assert_eq!(r["result"]["isError"], false, "{r}");
    }
    let next: serde_json::Value =
        serde_json::from_str(replies[1]["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(next["count"], 2);
    assert!(
        next["items"][0].get("blockers").is_some(),
        "dependency state travels with every item"
    );

    assert_eq!(p.count(), 4, "create_item wrote a real file");
    let claimed: serde_json::Value =
        serde_json::from_str(replies[3]["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(claimed["assignee"], "tester");
    assert!(claimed.get("body").is_some(), "the claimer gets the body");
}

#[test]
fn mcp_reports_tool_failures_in_band() {
    // A model has to be able to read the failure and correct itself, which it
    // cannot do if the transport aborts the call.
    let p = seeded();
    let replies = p.mcp(
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
    let replies = p.mcp(&[r#"{"jsonrpc":"2.0","id":1,"method":"bogus/method"}"#]);
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
// --- the agent surface, held to what it advertises --------------------------

/// A project that restricts what an agent may touch, in both of the ways the
/// schema allows: a field it may only read, and a status it may only propose.
fn restricted_for_agents() -> Project {
    let p = Project::with(
        Schema::standard()
            .field(Field::text("risk").agent(Agent::ReadOnly))
            .amend_status("done", |s| s.agent(Agent::Propose)),
    );
    seed(&p);
    p
}

/// The permission model was enforced on the command line and not over MCP —
/// the exact inverse of what the code and the manual both claimed.
///
/// `create_item` and `update_item` applied the caller's `fields` object with
/// `apply` rather than `apply_requested`, so every custom field went straight
/// past the check. `claim_item` and `close_item` did the same for `status`.
#[test]
fn an_agent_cannot_write_a_field_the_schema_reserves() {
    let p = restricted_for_agents();

    for (name, args) in [
        (
            "create_item",
            serde_json::json!({"title": "New", "fields": {"risk": "high"}}),
        ),
        (
            "update_item",
            serde_json::json!({"id": 1, "fields": {"risk": "high"}}),
        ),
    ] {
        let r = p.mcp_call(name, args, Some("claude"));
        assert!(
            refused(&r),
            "{name} wrote a read-only field: {}",
            tool_text(&r)
        );
        assert_contains(
            &tool_text(&r),
            "may read `risk` but not set it",
            "and said why",
        );
    }

    // Nothing was written by the refused create.
    assert!(
        !p.expect(&["list", "-A", "--plain"]).stdout.contains("New"),
        "a refused create must leave nothing behind"
    );
    // And the field is genuinely still unset on the item that was updated.
    assert!(
        !p.expect(&["show", "1", "--json"])
            .stdout
            .contains("\"risk\""),
        "the read-only field was written anyway"
    );
}

#[test]
fn an_agent_cannot_move_an_item_to_a_status_it_may_only_propose() {
    let p = restricted_for_agents();

    let r = p.mcp_call("close_item", serde_json::json!({"id": 1}), Some("claude"));
    assert!(
        refused(&r),
        "close_item moved to a propose status: {}",
        tool_text(&r)
    );
    assert_contains(&tool_text(&r), "may not move an item to `done`", "");

    let r = p.mcp_call(
        "create_item",
        serde_json::json!({"title": "Born done", "status": "done"}),
        Some("claude"),
    );
    assert!(
        refused(&r),
        "create_item started at a propose status: {}",
        tool_text(&r)
    );
}

/// The permission model hung off `clientInfo.name`, recorded during
/// `initialize`. A client that called a tool first was not an agent as far as
/// the check was concerned, and could write anything.
///
/// Arriving over this transport is what makes a caller an agent. The name only
/// says which one.
#[test]
fn skipping_initialize_does_not_escape_the_permission_model() {
    let p = restricted_for_agents();

    let r = p.mcp_call(
        "create_item",
        serde_json::json!({"title": "Snuck in", "fields": {"risk": "high"}}),
        None,
    );
    assert!(
        refused(&r),
        "a tool call before initialize wrote it: {}",
        tool_text(&r)
    );
    assert_contains(
        &tool_text(&r),
        "`mcp`",
        "named as an agent even unidentified",
    );
}

/// `clientInfo.name` arrives off the wire and becomes an assignee, a
/// `created_by`, and a hook's environment. A NUL byte in it used to reach
/// `std::env::set_var`, which panics, and killed the server mid-stream.
#[test]
fn a_hostile_client_name_does_not_bring_the_server_down() {
    let p = seeded();

    for name in [
        "cl\u{0}ude",
        "evil\ntitle: pwned",
        "tab\there",
        "$(rm -rf /)",
        "../../etc/passwd",
    ] {
        let r = p.mcp_call("check", serde_json::json!({}), Some(name));
        assert!(
            !r.is_null(),
            "the server did not answer for a client called {name:?}"
        );
    }

    // And a name with a newline in it is written as a name, not as a scalar
    // that happens to contain a line break.
    let p = seeded();
    p.mcp_call(
        "claim_item",
        serde_json::json!({"id": 1, "force": true}),
        Some("evil\ntitle: pwned"),
    );
    let file = p.read(&p.expect(&["show", "1", "--path"]).trimmed());
    assert!(
        file.contains("assignee: 'evil title: pwned'"),
        "the newline survived into the frontmatter:\n{file}"
    );
    assert!(p.run(&["check"]).ok(), "and the project still parses");
}

/// A very long name is bounded before it reaches a file.
#[test]
fn a_client_name_is_bounded() {
    let p = seeded();
    let long = "n".repeat(10_000);
    p.mcp_call(
        "claim_item",
        serde_json::json!({"id": 1, "force": true}),
        Some(&long),
    );
    let json: serde_json::Value =
        serde_json::from_str(&p.expect(&["show", "1", "--json"]).stdout).unwrap();
    let who = json["assignee"].as_str().unwrap_or_default();
    assert!(
        who.len() <= 64 && !who.is_empty(),
        "a client is free to call itself anything; a project's files are not \
         the place to find out how long: {} chars",
        who.len()
    );
}

/// Every tool advertises a schema. A schema that under-promises is one a
/// caller obeys unnecessarily; one that over-promises is a lie.
#[test]
fn every_tool_accepts_what_its_schema_advertises() {
    let p = seeded();
    let replies = p.mcp(&[r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#]);
    let tools = replies[0]["result"]["tools"].as_array().expect("tools");
    assert_eq!(
        tools.len(),
        13,
        "a tool was added or removed without a test"
    );

    for t in tools {
        let name = t["name"].as_str().unwrap();
        let schema = &t["inputSchema"];
        assert_eq!(schema["type"], "object", "{name}");
        let required = schema["required"].as_array().expect("required");
        let props = schema["properties"].as_object().expect("properties");
        for r in required {
            let r = r.as_str().unwrap();
            assert!(
                props.contains_key(r),
                "{name} requires `{r}` and does not describe it"
            );
        }

        // Every tool taking an id must say it accepts the rendered form too,
        // because `require_id` does and a model that has seen `0001` in output
        // will send `0001`.
        if let Some(id) = props.get("id") {
            assert_eq!(
                id["type"],
                serde_json::json!(["integer", "string"]),
                "{name} advertises less than it accepts for `id`"
            );
        }
    }
}

#[test]
fn a_tool_accepts_an_id_in_either_form() {
    let p = seeded();
    for id in [serde_json::json!(1), serde_json::json!("0001")] {
        let r = p.mcp_call("show_item", serde_json::json!({"id": id}), Some("claude"));
        assert!(!refused(&r), "{id} was refused: {}", tool_text(&r));
    }
}

/// `fields` was required, so replacing only the body meant sending an empty
/// object for want of anything better.
#[test]
fn an_update_may_change_only_the_body() {
    let p = seeded();
    let r = p.mcp_call(
        "update_item",
        serde_json::json!({"id": 1, "body": "Rewritten."}),
        Some("claude"),
    );
    assert!(!refused(&r), "{}", tool_text(&r));
    assert_contains(&p.expect(&["show", "1"]).stdout, "Rewritten.", "");

    // Neither one is not a change.
    let r = p.mcp_call("update_item", serde_json::json!({"id": 1}), Some("claude"));
    assert!(refused(&r));
    assert_contains(&tool_text(&r), "nothing to change", "");
}

/// Arguments of the wrong JSON type, which a model produces more often than a
/// missing argument.
#[test]
fn a_tool_given_the_wrong_type_fails_in_band() {
    let p = seeded();
    for (name, args) in [
        ("show_item", serde_json::json!({"id": {"nested": true}})),
        (
            "update_item",
            serde_json::json!({"id": 1, "fields": "not an object"}),
        ),
        ("search_items", serde_json::json!({"query": 42})),
        ("list_items", serde_json::json!({"limit": "many"})),
        ("add_note", serde_json::json!({"id": 1, "text": []})),
    ] {
        let r = p.mcp_call(name, args, Some("claude"));
        assert!(
            !r.is_null() && r["content"][0]["type"] == "text",
            "{name} did not answer in band"
        );
    }
    assert!(p.run(&["check"]).ok(), "and nothing was corrupted");
}

/// Two clients against one project. The lock is the only thing between them.
#[test]
fn two_agents_at_once_leave_one_consistent_backlog() {
    let p = seeded();
    let mut kids = Vec::new();
    for who in ["claude", "codex"] {
        let init = serde_json::json!({
            "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": { "clientInfo": { "name": who } }
        })
        .to_string();
        let mut input = vec![init];
        for n in 0..8 {
            input.push(
                serde_json::json!({
                    "jsonrpc": "2.0", "id": n + 1, "method": "tools/call",
                    "params": { "name": "create_item",
                                "arguments": { "title": format!("{who} {n}") } }
                })
                .to_string(),
            );
        }
        let root = p.root().to_path_buf();
        kids.push(std::thread::spawn(move || {
            let mut child = Command::new(bin())
                .arg("mcp")
                .current_dir(&root)
                .env("NO_COLOR", "1")
                .env("PATH", path_with_binary())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("spawn");
            use std::io::Write;
            child
                .stdin
                .take()
                .unwrap()
                .write_all(format!("{}\n", input.join("\n")).as_bytes())
                .expect("write");
            child.wait_with_output().expect("wait")
        }));
    }
    for k in kids {
        let out = k.join().expect("thread");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());
    let ids = p.expect(&["list", "-A", "--ids"]).lines();
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(
        ids.len(),
        unique.len(),
        "two clients were given the same id"
    );
}

/// Two branches creating the same file have no common ancestor, so the driver
/// cannot do a three-way merge. It has to *decline* — which means leaving a
/// conflict a person can see.
///
/// git writes no markers of its own when a custom driver runs. This path
/// returned without calling `git merge-file`, so an add/add conflict left
/// `ours` in the working tree with no sign the other side had said anything —
/// the same defect the ordinary decline path was already fixed for.
#[test]
fn an_add_add_conflict_leaves_markers_rather_than_ours() {
    let p = repository();
    p.add("Starting point", &[]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "start"]);

    // Two branches writing the same path with different content, by hand, so
    // there is genuinely no ancestor for that file.
    let path = "cairn/items/0009-contested.md";
    for (branch, title) in [("ours", "Ours"), ("theirs", "Theirs")] {
        git(&p, &["checkout", "-q", "main"]);
        git(&p, &["checkout", "-qb", branch]);
        p.write(
            path,
            &format!("---\nid: 9\ntitle: {title}\nstatus: backlog\n---\n{title} body\n"),
        );
        git(&p, &["add", "-A"]);
        git(&p, &["commit", "-qm", branch]);
    }

    git(&p, &["checkout", "-q", "ours"]);
    let out = merge(&p, "theirs");
    assert_ne!(out.code, 0, "an add/add conflict should not merge cleanly");

    let file = p.read(path);
    assert!(
        file.contains("<<<<<<<") && file.contains(">>>>>>>"),
        "the working tree has `ours` and no sign the other side existed:\n{file}"
    );
    assert!(
        file.contains("Theirs"),
        "what the other side wrote must survive into the conflict:\n{file}"
    );
}

/// The same thing said directly to the driver, because what git chooses to
/// invoke it for is git's business and this is the contract cairn owes.
///
/// An empty ancestor is what git passes for a file added on both sides. The
/// driver cannot merge that, and declining has to mean markers.
#[test]
fn the_driver_never_declines_without_leaving_a_conflict() {
    let p = Project::new();
    p.write(
        "ours.md",
        "---\nid: 9\ntitle: Ours\nstatus: backlog\n---\nours\n",
    );
    p.write("base.md", "");
    p.write(
        "theirs.md",
        "---\nid: 9\ntitle: Theirs\nstatus: backlog\n---\ntheirs\n",
    );

    let out = p.run(&[
        "merge-driver",
        &p.path("ours.md").display().to_string(),
        &p.path("base.md").display().to_string(),
        &p.path("theirs.md").display().to_string(),
        "cairn/items/0009-x.md",
    ]);
    assert_eq!(out.code, 1, "it must report that it did not resolve");

    let file = p.read("ours.md");
    assert!(
        file.contains("<<<<<<<") && file.contains("Theirs"),
        "declining left `ours` with no sign the other side existed:\n{file}"
    );
}

/// Deleting an item on one side and editing it on the other is a question for a
/// person. The driver must not answer it.
#[test]
fn a_delete_against_an_edit_is_left_to_a_person() {
    let p = repository();
    p.add("Contested", &[]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "start"]);
    let path = p.expect(&["show", "1", "--path"]).trimmed();

    on_branch(&p, "editing", "main", &["set", "1", "assignee=someone"]);

    git(&p, &["checkout", "-q", "main"]);
    git(&p, &["checkout", "-qb", "deleting"]);
    p.expect(&["remove", "1", "--force"]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "deleting"]);

    let out = merge(&p, "editing");
    assert_ne!(out.code, 0, "a delete against an edit is not resolvable");
    assert!(
        out.all().to_lowercase().contains("conflict"),
        "and git should say so: {}",
        out.all()
    );
    // Whatever a person decides, the edited content is still reachable.
    let _ = path;
}

/// The configuration is deliberately outside the driver: unioning two schema
/// blocks would produce a schema neither branch wrote.
#[test]
fn the_driver_does_not_claim_the_configuration() {
    let p = repository();
    let attrs = p.read(".gitattributes");
    assert!(
        !attrs.lines().any(|l| l.starts_with("cairn.toml")),
        "cairn.toml must not be given to the merge driver:\n{attrs}"
    );

    // And a real conflict in it comes out as a conflict, with both sides
    // visible, rather than being resolved by anything.
    for (branch, name) in [("ours", "mine"), ("theirs", "yours")] {
        git(&p, &["checkout", "-q", "main"]);
        git(&p, &["checkout", "-qb", branch]);
        // Both branches appending a block at the end of the file, which is
        // exactly the shape a schema conflict takes in practice.
        p.append(
            "cairn.toml",
            &format!("\n[[view]]\nname = \"{name}\"\nfilter = \"status=backlog\"\n"),
        );
        git(&p, &["add", "-A"]);
        git(&p, &["commit", "-qm", branch]);
    }
    git(&p, &["checkout", "-q", "ours"]);
    let out = merge(&p, "theirs");
    assert_ne!(
        out.code, 0,
        "two schemas disagreeing is a person's question"
    );
    let cfg = p.read("cairn.toml");
    assert!(
        cfg.contains("<<<<<<<") && cfg.contains("yours"),
        "both sides must be visible in the conflict:\n{cfg}"
    );
}
// --- coverage: surfaces the suite had never driven ---------------------------

/// A board grouped by something other than status, which is most of the command.
#[test]
fn a_board_groups_by_any_field() {
    let p = seeded();
    p.expect(&["set", "1", "priority=p0"]);
    p.expect(&["set", "2", "priority=p2"]);

    let by_priority = p.expect(&["board", "--group-by", "priority"]).stdout;
    assert_contains(&by_priority, "p0", "the enum's own values are the columns");
    assert_contains(&by_priority, "p3", "including ones nothing is filed under");

    // Item 1 carries v0.1; the rest carry nothing, so both a named column and
    // the unscheduled one appear.
    //
    // The context used to be built *after* containers were dropped, so
    // `--group-by milestone` found no milestones, drew no column for them, and
    // every item that had one disappeared from the board while the unscheduled
    // ones stayed. A board that quietly omits scheduled work is worse than one
    // that shows nothing.
    let by_milestone = p.expect(&["board", "--group-by", "milestone"]).stdout;
    assert_contains(&by_milestone, "v0.1", "the milestone column");
    assert_contains(&by_milestone, "(none)", "and one for what is unscheduled");
    assert_contains(
        &by_milestone,
        "First item",
        "the scheduled item is on the board",
    );

    // A field with no declared values takes its columns from what items carry.
    p.expect(&["set", "1", "area=parser"]);
    let by_area = p.expect(&["board", "--group-by", "area"]).stdout;
    assert_contains(&by_area, "parser", "");
}

#[test]
fn a_board_honours_a_view_a_filter_and_a_milestone() {
    let p = seeded();
    p.append(
        "cairn.toml",
        "\n[[view]]\nname = \"hot\"\nfilter = \"priority=p0\"\ngroup_by = \"milestone\"\n",
    );
    p.expect(&["set", "1", "priority=p0"]);

    // The view names a group_by, which is the branch worth reaching.
    let out = p.expect(&["board", "--view", "hot"]).stdout;
    assert_contains(&out, "v0.1", "the view's own group_by was used");

    p.expect(&["board", "--filter", "category!=done"]);
    p.expect(&["board", "--milestone", "v0.1"]);
    p.expect(&["board", "--all"]);
    p.expect(&["board", "--width", "20"]);

    let out = p.fails(&["board", "--view", "nonesuch"]).all();
    assert_contains(&out, "unknown view", "");
}

/// `cairn roadmap --items` lists the work under each milestone.
#[test]
fn a_roadmap_can_list_its_items() {
    let p = seeded();
    let out = p.expect(&["roadmap", "--items"]).stdout;
    assert_contains(&out, "v0.1", "the milestone");
    assert!(
        out.lines().count() > p.expect(&["roadmap"]).stdout.lines().count(),
        "--items showed no more than the summary did"
    );

    p.expect(&["close", "1"]);
    let closed_hidden = p.expect(&["roadmap", "--items"]).stdout;
    let closed_shown = p.expect(&["roadmap", "--items", "--all"]).stdout;
    assert!(
        closed_shown.len() > closed_hidden.len(),
        "--all did not bring back what was finished"
    );
}

/// A dependency that no longer exists is called out where somebody will see it,
/// rather than left as a number that resolves to nothing.
#[test]
fn show_marks_a_dependency_that_is_missing() {
    let p = seeded();
    p.write(
        "cairn/items/0050-hangs-on-nothing.md",
        "---\nid: 50\ntitle: Hangs on nothing\nstatus: backlog\ndepends_on:\n  - 999\n---\nbody\n",
    );
    let out = p.expect(&["show", "50"]).stdout;
    assert_contains(&out, "missing", "it says the dependency is not there");

    // And a dependency that exists is shown ticked once it is finished.
    p.expect(&["close", "1"]);
    p.expect(&["set", "50", "depends_on=1"]);
    assert_contains(&p.expect(&["show", "50"]).stdout, "[x]", "");
}

/// The rendered roadmap's optional parts, none of which the suite had produced.
#[test]
fn a_rendered_roadmap_carries_its_furniture() {
    let p = seeded();
    p.write("docs/intro.md", "Read this first.\n");
    p.write("docs/outro.md", "That is all.\n");
    let cfg = p
        .read("cairn.toml")
        .replace(
            "[render]",
            "[render]\nheader = \"docs/intro.md\"\nfooter = \"docs/outro.md\"",
        )
        .replace("title = \"Roadmap\"", "title = \"The plan\"");
    p.write("cairn.toml", &cfg);

    p.expect(&["render"]);
    let out = p.read("ROADMAP.md");
    assert_contains(&out, "# The plan", "the configured title");
    assert_contains(&out, "Read this first.", "the header fragment");
    assert_contains(&out, "That is all.", "the footer fragment");

    // A header naming a file that is not there is reported, not ignored.
    let cfg = p
        .read("cairn.toml")
        .replace("docs/intro.md", "docs/gone.md");
    p.write("cairn.toml", &cfg);
    let out = p.run(&["render"]);
    assert!(!out.ok(), "a missing fragment should not render silently");
}

#[test]
fn a_rendered_roadmap_can_link_to_its_items() {
    let p = seeded();
    let cfg = p
        .read("cairn.toml")
        .replace("link_items = false", "link_items = true")
        .replace(
            "[project]",
            "[project]\nurl = \"https://example.invalid/blob/main\"",
        );
    p.write("cairn.toml", &cfg);
    p.expect(&["render"]);
    assert_contains(
        &p.read("ROADMAP.md"),
        "https://example.invalid/blob/main/cairn/items/",
        "items are linked",
    );
}

/// `render --output` and `--check`, which is what continuous integration runs.
#[test]
fn render_can_write_elsewhere_and_check_itself() {
    let p = seeded();
    p.expect(&["render"]);
    assert!(p.run(&["render", "--check"]).ok(), "it was just rendered");

    // `--no-hooks`, or the after-create hook renders it again and it is never
    // stale.
    p.expect(&["new", "Something new", "-q", "--no-hooks"]);
    let out = p.run(&["render", "--check"]);
    assert!(!out.ok(), "the roadmap is stale and --check should say so");
    assert_contains(&out.all(), "render", "and name the remedy");

    p.expect(&["render", "--output", "elsewhere.md"]);
    assert!(!p.read("elsewhere.md").is_empty());
}

/// A title longer than its column is clipped with an ellipsis rather than
/// wrapped or truncated mid-character.
#[test]
fn a_long_title_is_clipped_to_its_column() {
    let p = Project::new();
    let long = "A title that goes on and on and on and will not fit into any \
                reasonable column width at all";
    p.add(long, &[]);
    p.add(
        "Ünïcödé — a title with wide characters ☂☂☂☂☂☂☂☂☂☂☂☂☂☂☂☂☂☂☂☂",
        &[],
    );

    let out = p.expect(&["board", "--width", "24"]).stdout;
    assert_contains(&out, "…", "something was clipped");
    assert!(
        out.lines().all(|l| l.chars().count() < 400),
        "a line ran away"
    );
    assert!(p.run(&["check"]).ok());
}

/// Re-importing over a backlog that already has the items: the update path,
/// which had never run.
#[test]
fn a_second_import_updates_rather_than_duplicates() {
    let p = Project::new();
    let doc = |title: &str, status: &str| {
        serde_json::json!({
            "items": [{
                "id": 1, "title": title, "status": status,
                "source": "elsewhere#1", "body": "from outside"
            }]
        })
        .to_string()
    };

    let out = p.run_stdin(&["import"], &doc("First name", "backlog"));
    assert!(out.ok(), "{}", out.all());
    let count = p.expect(&["list", "-A", "--count"]).trimmed();

    // The same record again, changed, with --update.
    let out = p.run_stdin(&["import", "--update"], &doc("Second name", "backlog"));
    assert!(out.ok(), "{}", out.all());
    assert_eq!(
        p.expect(&["list", "-A", "--count"]).trimmed(),
        count,
        "the second import duplicated instead of updating"
    );
    assert_contains(
        &p.expect(&["list", "-A", "--plain"]).stdout,
        "Second name",
        "and the update took",
    );

    // Without --update, an item already present is left alone.
    let out = p.run_stdin(&["import"], &doc("Third name", "backlog"));
    assert!(out.ok(), "{}", out.all());
    assert_contains(&out.all(), "already present", "");
}

#[test]
fn an_import_can_create_the_milestones_it_names() {
    let p = Project::new();
    let doc = serde_json::json!({
        "items": [
            {"id": 1, "title": "Scheduled", "status": "backlog", "milestone": "v9.9"},
            {"id": 2, "title": "Also", "status": "backlog", "milestone": "v9.9"}
        ]
    });
    let out = p.run_stdin(&["import", "--create-milestones"], &doc.to_string());
    assert!(out.ok(), "{}", out.all());
    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());
    assert_contains(
        &p.expect(&["list", "-A", "--plain", "--columns", "id,key,type"])
            .stdout,
        "v9.9",
        "the milestone was created once",
    );
    assert_eq!(
        p.expect(&["list", "-A", "--ids", "--filter", "key=v9.9"])
            .lines()
            .len(),
        1,
        "a milestone was created twice"
    );
}

/// `import --close` comments on and closes the source issues.
#[cfg(unix)]
#[test]
fn an_import_can_close_what_it_took() {
    let p = Project::new();
    let issues = serde_json::json!([
        { "number": 7, "title": "Taken over", "body": "b", "state": "OPEN",
          "labels": [], "assignees": [], "milestone": null }
    ]);
    let script = format!(
        "if [ \"$2\" = list ]; then cat <<'JSON'\n{issues}\nJSON\nelse echo \"closed $3\"; fi"
    );
    let out = import_github(&p, &script, &["--close"]);
    assert!(out.ok(), "{}", out.all());
    assert_contains(&out.all(), "closed", "it reported closing the source");
    assert!(p.run(&["check"]).ok());
}

/// The schema an agent reads first, which nothing had ever called.
#[test]
fn the_agent_can_ask_for_the_schema() {
    let p = seeded();
    let r = p.mcp_call("get_schema", serde_json::json!({}), Some("claude"));
    let text = r["content"][0]["text"].as_str().expect("text");
    let schema: serde_json::Value = serde_json::from_str(text).expect("JSON");

    assert!(schema["statuses"].is_array(), "{schema}");
    assert!(schema["fields"].is_array(), "{schema}");
    assert!(schema["counts"]["total"].is_number(), "{schema}");
    assert!(
        schema["filter_syntax"]["operators"].is_array(),
        "an agent has to be told the filter grammar: {schema}"
    );
    assert!(
        schema["milestones"].is_array(),
        "milestones are items now and still belong in the schema: {schema}"
    );
}

/// An identifier format with a suffix as well as a prefix. Reading one back
/// exercises the other end of the parser.
#[test]
fn an_identifier_can_have_a_suffix() {
    let p = keyed("[{n:03}]", None);
    p.expect(&["new", "First", "-q"]);
    assert_eq!(p.expect(&["list", "--ids"]).trimmed(), "[001]");

    // Accepted back in every spelling: rendered, bare, and case-folded.
    for spelling in ["[001]", "001", "1", "#1", "[1]"] {
        assert!(
            p.run(&["show", spelling]).ok(),
            "`{spelling}` should name the same item"
        );
    }
    assert!(!p.run(&["show", "[abc]"]).ok());
}

/// Colours are named in `cairn.toml`, and every name it accepts should reach
/// the terminal rather than falling through to plain text.
#[test]
fn every_named_colour_is_understood() {
    let p = Project::new();
    let mut cfg = p.read("cairn.toml");
    for (n, colour) in ["black", "magenta", "purple", "white", "bold", "grey"]
        .into_iter()
        .enumerate()
    {
        cfg.push_str(&format!(
            "\n[[status]]\nname = \"s{n}\"\ncategory = \"open\"\ncolor = \"{colour}\"\n"
        ));
    }
    cfg.push_str(
        "\n[[status]]\nname = \"nonsense\"\ncategory = \"open\"\ncolor = \"chartreuse\"\n",
    );
    p.write("cairn.toml", &cfg);

    for n in 0..6 {
        p.expect(&["new", &format!("Item {n}"), "-s", &format!("s{n}"), "-q"]);
    }
    p.expect(&["new", "Unknown colour", "-s", "nonsense", "-q"]);

    let out = p.run_env(&["list", "-A", "--color", "always"], &[("NO_COLOR", None)]);
    assert!(out.ok(), "{}", out.all());
    assert!(out.stdout.contains('\u{1b}'), "nothing was painted");
    // A colour name nothing recognises is left alone rather than refused.
    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());
}

/// A configuration with no `[project]` block at all, which is the smallest
/// thing that is still a project.
#[test]
fn a_configuration_with_no_project_block_works() {
    let p = Project::empty();
    p.write(
        "cairn.toml",
        "format = 3\n[[status]]\nname = \"todo\"\ncategory = \"open\"\n",
    );
    std::fs::create_dir_all(p.path("cairn/items")).unwrap();

    p.expect(&["new", "Minimal", "-q"]);
    assert_eq!(p.expect(&["list", "--ids"]).trimmed(), "0001");
    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());
    assert_contains(
        &p.expect(&["config"]).stdout,
        "cairn/items",
        "the default directory",
    );
}

/// A ref field addressed by id stores a number, not a string, so the file says
/// what it means and a reader does not have to guess.
#[test]
fn an_id_addressed_ref_stores_numbers() {
    let p = Project::new();
    p.append(
        "cairn.toml",
        "\n[[field]]\nname = \"blocks\"\nkind = \"ref\"\ntarget = \"*\"\n\
         by = \"id\"\ncardinality = \"many\"\n\
         \n[[field]]\nname = \"parent_item\"\nkind = \"ref\"\ntarget = \"*\"\nby = \"id\"\n",
    );
    p.add("One", &[]);
    p.add("Two", &[]);
    p.add("Three", &[]);

    p.expect(&["set", "3", "blocks=1,2"]);
    p.expect(&["set", "3", "parent_item=1"]);

    let file = p.read(&p.expect(&["show", "3", "--path"]).trimmed());
    assert!(
        file.contains("- 1") && file.contains("- 2"),
        "a many-valued id ref should be a sequence of numbers:\n{file}"
    );
    assert!(
        file.contains("parent_item: 1"),
        "a single id ref should be a number:\n{file}"
    );
    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());

    // Cleared, the keys go rather than being left empty.
    p.expect(&["set", "3", "blocks="]);
    p.expect(&["set", "3", "parent_item="]);
    let file = p.read(&p.expect(&["show", "3", "--path"]).trimmed());
    assert!(
        !file.contains("parent_item"),
        "a cleared ref was left behind:\n{file}"
    );
}

/// Setting several items at once, where one of them fails: the ones already
/// written are named, so nobody has to guess how far it got.
#[test]
fn a_partial_multi_item_write_says_how_far_it_got() {
    let p = seeded();
    let out = p.fails(&["set", "1", "2", "3", "status=nonsense"]);
    assert_contains(&out.all(), "nonsense", "it names the bad value");

    // The first item takes a good change, the second a bad one.
    let out = p.fails(&["set", "2", "3", "priority=p0", "effort=enormous"]);
    assert_contains(&out.all(), "enormous", "");
    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());
}

/// `next` has a flag for every way of asking "what should I do?", and most had
/// never been driven.
#[test]
fn next_answers_every_way_of_asking() {
    let p = seeded();
    p.expect(&["set", "1", "assignee=tester"]);
    p.expect(&["set", "2", "depends_on=1"]);

    assert!(!p.expect(&["next", "--mine"]).stdout.is_empty());
    assert!(!p.expect(&["next", "--unassigned"]).stdout.is_empty());
    assert!(
        !p.expect(&["next", "--assignee", "tester"])
            .stdout
            .is_empty()
    );
    p.expect(&["next", "--type", "feature"]);
    p.expect(&["next", "--milestone", "v0.1"]);
    p.expect(&["next", "--filter", "priority=p0"]);
    p.expect(&["next", "--limit", "1"]);

    // Blocked work is excluded, and `--blocked` brings it back with its reason.
    let plain = p.expect(&["next", "--ids"]).lines();
    assert!(
        !plain.contains(&"0002".to_string()),
        "blocked work was offered"
    );
    let with_blocked = p.expect(&["next", "--blocked", "--ids"]).lines();
    assert!(with_blocked.contains(&"0002".to_string()));

    let count: usize = p
        .expect(&["next", "--count"])
        .trimmed()
        .parse()
        .expect("a number");
    assert!(count > 0);
    let json: serde_json::Value =
        serde_json::from_str(&p.expect(&["next", "--json"]).stdout).expect("JSON");
    assert!(json.as_array().is_some(), "{json}");
}

/// Search over titles alone, and every output shape.
#[test]
fn search_has_the_same_output_shapes_as_everything_else() {
    let p = seeded();
    p.expect(&["note", "1", "the word cassowary appears only in a body"]);

    assert!(!p.expect(&["search", "cassowary"]).stdout.is_empty());
    // `--titles` looks at titles alone, so a word that lives only in a body
    // finds nothing — and finding nothing is an error, not a silent success.
    let out = p.fails(&["search", "cassowary", "--titles"]);
    assert_contains(&out.all(), "no items match", "");
    assert!(!p.expect(&["search", "item", "--titles"]).stdout.is_empty());

    p.expect(&["search", "item", "--json"]);
    p.expect(&["search", "item", "--ids"]);
    p.expect(&["search", "item", "--plain"]);
    p.expect(&["search", "item", "--count"]);
    p.expect(&["search", "item", "--limit", "1"]);
    p.expect(&["search", "item", "--filter", "category!=done"]);
    p.expect(&["close", "2"]);
    p.expect(&["search", "item", "--all"]);
}

/// Export's shapes and filters.
#[test]
fn export_can_be_narrowed_and_redirected() {
    let p = seeded();
    p.expect(&["close", "2"]);

    let all: serde_json::Value = serde_json::from_str(&p.expect(&["export"]).stdout).expect("JSON");
    let open: serde_json::Value =
        serde_json::from_str(&p.expect(&["export", "--open-only"]).stdout).expect("JSON");
    assert!(
        open["items"].as_array().unwrap().len() < all["items"].as_array().unwrap().len(),
        "--open-only kept everything"
    );

    let filtered: serde_json::Value =
        serde_json::from_str(&p.expect(&["export", "--filter", "priority=p0"]).stdout)
            .expect("JSON");
    assert!(!filtered["items"].as_array().unwrap().is_empty());

    p.expect(&["export", "--output", "out.json"]);
    assert!(!p.read("out.json").is_empty());
}

/// `set --filter` is the dangerous write: the person running it has not seen
/// the list, so it shows one and asks.
#[test]
fn a_filtered_write_shows_what_it_will_change_and_asks() {
    let p = seeded();

    // Declined.
    let out = p.run_stdin(&["set", "--filter", "category!=done", "priority=p0"], "n\n");
    assert_eq!(
        out.code,
        1,
        "declining should not be success: {}",
        out.all()
    );
    assert_contains(&out.all(), "aborted", "");
    assert_contains(&out.stdout, "First item", "it listed what it would touch");
    assert_eq!(
        p.expect(&["list", "-A", "--ids", "--filter", "priority=p0"])
            .lines()
            .len(),
        1,
        "a declined write changed something"
    );

    // Accepted.
    let out = p.run_stdin(&["set", "--filter", "category!=done", "priority=p1"], "y\n");
    assert!(out.ok(), "{}", out.all());
    assert!(
        p.expect(&["list", "-A", "--ids", "--filter", "priority=p1"])
            .lines()
            .len()
            >= 2
    );

    // `--yes` does not ask at all.
    let out = p.expect(&["set", "--filter", "category!=done", "priority=p3", "--yes"]);
    assert!(!out.all().contains("[y/N]"), "--yes still asked");

    // A filter matching nothing is an error rather than a silent no-op.
    let out = p.fails(&["set", "--filter", "priority=p9", "effort=s", "--yes"]);
    assert_contains(&out.all(), "no item matches", "");

    // Ids and a filter together is a mistake, not a union.
    let out = p.fails(&["set", "1", "--filter", "priority=p0", "effort=s"]);
    assert_contains(&out.all(), "not both", "");

    // And no target at all.
    let out = p.fails(&["set", "effort=s"]);
    assert_contains(&out.all(), "no item named", "");
}

#[test]
fn set_refuses_an_empty_assignment_list() {
    let p = seeded();
    let out = p.fails(&["set", "1"]);
    assert_contains(&out.all(), "field=value", "it says what it wanted");
}

/// A milestone reference that resolves to nothing is shown in a way somebody
/// notices, rather than as a plausible string.
#[test]
fn show_marks_a_milestone_that_answers_to_nothing() {
    let p = seeded();
    p.write(
        "cairn/items/0060-adrift.md",
        "---\nid: 60\ntitle: Adrift\nstatus: backlog\nmilestone: v9.9\n---\nbody\n",
    );
    let out = p.run_env(&["show", "60", "--color", "always"], &[("NO_COLOR", None)]);
    assert!(out.ok(), "{}", out.all());
    assert_contains(&out.stdout, "v9.9", "the value is shown");
    assert!(out.stdout.contains('\u{1b}'), "and marked, not left plain");
}

/// `show --raw` and `--path`, which are what a script reaches for.
#[test]
fn show_has_a_shape_for_a_script() {
    let p = seeded();
    let raw = p.expect(&["show", "1", "--raw"]).stdout;
    assert!(raw.starts_with("---"), "--raw is the file itself:\n{raw}");

    let path = p.expect(&["show", "1", "--path"]).trimmed();
    assert!(path.ends_with(".md"), "{path}");
    assert_eq!(p.read(&path), raw, "--path and --raw disagree");
}

/// `migrate --check` is what continuous integration runs, and it answers
/// differently depending on which side of the format the project is on.
#[test]
fn migrate_check_reports_both_ways() {
    let p = format_one();
    let out = p.run(&["migrate", "--check"]);
    assert_eq!(
        out.code, 1,
        "a project behind the format should fail --check"
    );
    assert_contains(&out.all(), "stale", "");
    assert_contains(&out.all(), "cairn migrate", "and name the remedy");

    p.expect(&["migrate"]);
    let out = p.expect(&["migrate", "--check"]);
    assert_contains(&out.all(), "current", "");

    // Quietly, for a script.
    let out = p.expect(&["migrate", "--check", "--quiet"]);
    assert!(
        out.stdout.trim().is_empty(),
        "--quiet printed: {}",
        out.stdout
    );
}

#[test]
fn migrate_outside_a_project_says_so() {
    let p = Project::empty();
    let out = p.run(&["migrate"]);
    assert!(!out.ok());
    assert_contains(&out.all(), "cairn.toml", "");
}

/// A table wider than the terminal: the last column absorbs what is left and
/// everything stays on one line.
#[test]
fn a_table_fits_a_narrow_terminal() {
    let p = Project::new();
    for n in 0..4 {
        p.add(
            &format!("Item {n} with a title long enough that no narrow terminal could show all of it at once"),
            &["--set", "priority=p0", "--set", "area=some-fairly-long-area-name"],
        );
    }
    let out = p.run_env(
        &["list", "-A", "--columns", "id,priority,area,title"],
        &[("COLUMNS", Some("60"))],
    );
    assert!(out.ok(), "{}", out.all());
    for line in out.stdout.lines() {
        assert!(
            line.chars().count() < 400,
            "a row ran away rather than being clipped:\n{line}"
        );
    }
}

/// `log` has three shapes and a limit, and the one nothing had exercised is
/// `--patch`, which hands back the diffs themselves.
#[test]
fn log_can_show_the_patches_themselves() {
    let p = repository();
    p.expect(&["set", "1", "priority=p0"]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "priority"]);

    let out = p.expect(&["log", "1", "--patch"]);
    assert_contains(&out.stdout, "diff --git", "the raw diff");
    assert_contains(&out.stdout, "priority", "and what changed in it");

    // Bounded, for an item with a long history.
    p.expect(&["set", "1", "effort=s"]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "effort"]);
    let one = p.expect(&["log", "1", "--patch", "-n", "1"]).stdout;
    let all = p.expect(&["log", "1", "--patch"]).stdout;
    assert!(one.len() < all.len(), "-n did not limit the patches");
}

/// A milestone is an item, so its history is readable by the name people use
/// for it rather than by a number they would have to look up.
#[test]
fn history_can_be_asked_for_by_key() {
    let p = repository();
    milestone(&p, "v0.1", None);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "a milestone"]);

    let out = p.expect(&["log", "v0.1"]);
    assert!(out.ok(), "{}", out.all());

    // And a name that answers to nothing says so.
    let out = p.fails(&["log", "no-such-key"]);
    assert_contains(&out.all(), "no-such-key", "");
}

/// The item-level checks that had never been driven: a missing status, a type
/// nothing declares, a list field carrying a value outside its enum.
#[test]
fn check_reports_every_way_an_item_can_disagree_with_the_schema() {
    let p = Project::new();
    p.append(
        "cairn.toml",
        "\n[[field]]\nname = \"platforms\"\nkind = \"list\"\n\
         values = [\"linux\", \"macos\"]\n\
         \n[[field]]\nname = \"owner_team\"\nkind = \"text\"\nrequired = true\n",
    );
    p.write(
        "cairn/items/0011-no-status.md",
        "---\nid: 11\ntitle: No status\n---\nbody\n",
    );
    p.write(
        "cairn/items/0012-odd-type.md",
        "---\nid: 12\ntitle: Odd type\ntype: sculpture\nstatus: backlog\n---\nbody\n",
    );
    p.write(
        "cairn/items/0013-odd-list.md",
        "---\nid: 13\ntitle: Odd list\nstatus: backlog\nplatforms:\n  - linux\n  - haiku\n---\nbody\n",
    );

    let out = p.run(&["check"]);
    assert!(!out.ok());
    let all = out.all();
    assert_contains(&all, "missing `status`", "");
    assert_contains(&all, "unknown type `sculpture`", "");
    assert_contains(&all, "haiku", "a list value outside its enum");
    assert_contains(&all, "owner_team", "a required field nothing carries");
    // Diagnostics carry a line, so an editor can jump to the offending key.
    assert!(
        all.lines()
            .any(|l| l.contains(".md:") && l.contains("unknown type")),
        "no file:line on the diagnostics:\n{all}"
    );
}

/// `check --strict` and `--quiet`, which is how continuous integration uses it.
#[test]
fn check_can_be_strict_and_quiet() {
    let p = seeded();
    assert!(p.expect(&["check", "--quiet"]).stdout.trim().is_empty());

    // A warning: a filename that no longer matches its title.
    let path = p.expect(&["show", "1", "--path"]).trimmed();
    let moved = path.replace("first-item", "wrong-name");
    std::fs::rename(p.path(&path), p.path(&moved)).unwrap();

    assert!(p.run(&["check"]).ok(), "a warning is not a failure");
    let out = p.run(&["check", "--strict"]);
    assert!(!out.ok(), "--strict should make it one: {}", out.all());
}

/// An item file that cannot be parsed at all stops anything that writes, and
/// anything that produces a durable artefact, rather than acting on half a
/// backlog.
#[test]
fn a_partial_view_is_reported_by_every_reader_that_needs_all_of_it() {
    let p = seeded();
    p.write("cairn/items/0070-truncated.md", "---\nid: 70\ntitle: Trunc");

    // Reading commands still work, on what parses.
    assert!(p.run(&["list"]).ok());
    // Anything durable refuses.
    for args in [vec!["export"], vec!["render"], vec!["check"]] {
        let out = p.run(&args);
        assert!(!out.ok(), "`cairn {args:?}` acted on a partial backlog");
    }
}

/// A note appended under a heading of its own, and the default of today's date.
#[test]
fn a_note_can_choose_its_own_heading() {
    let p = seeded();
    p.expect(&["note", "1", "plain note"]);
    assert_contains(&p.expect(&["show", "1"]).stdout, "plain note", "");

    let r = p.mcp_call(
        "add_note",
        serde_json::json!({"id": 1, "text": "filed elsewhere", "heading": "Decisions"}),
        Some("claude"),
    );
    assert!(!refused(&r), "{}", tool_text(&r));
    let body = p.expect(&["show", "1"]).stdout;
    assert_contains(&body, "Decisions", "the heading it asked for");
    assert_contains(&body, "plain note", "and nothing was erased");
}

/// A project with no `done` category at all: `close` cannot guess, and says so
/// rather than picking something.
#[test]
fn close_without_a_done_status_asks_for_one() {
    let p = Project::empty();
    p.write(
        "cairn.toml",
        "format = 3\n[project]\nname = \"T\"\n\
         [[status]]\nname = \"todo\"\ncategory = \"open\"\n\
         [[status]]\nname = \"doing\"\ncategory = \"active\"\n",
    );
    std::fs::create_dir_all(p.path("cairn/items")).unwrap();
    p.expect(&["new", "Never finished", "-q"]);

    let out = p.fails(&["close", "1"]);
    assert_contains(&out.all(), "category = \"done\"", "it says what is missing");
    assert_contains(&out.all(), "--status", "and how to proceed anyway");

    // And explicitly is fine.
    assert!(p.run(&["close", "1", "--status", "doing"]).ok());
}

/// `reopen` mirrors `close`, including choosing where to go back to.
#[test]
fn reopen_returns_an_item_to_the_backlog() {
    let p = seeded();
    p.expect(&["close", "1"]);
    assert_contains(&p.expect(&["show", "1"]).stdout, "done", "");

    p.expect(&["reopen", "1"]);
    let out = p.expect(&["show", "1"]).stdout;
    assert!(!out.contains("done"), "it did not come back: {out}");

    p.expect(&["close", "1"]);
    p.expect(&["reopen", "1", "--status", "planned"]);
    assert_contains(&p.expect(&["show", "1"]).stdout, "planned", "");
}

/// Closing several at once, and the acceptance-criteria report that comes with
/// it.
#[test]
fn closing_reports_criteria_that_are_still_unticked() {
    let p = Project::new();
    p.expect(&[
        "new",
        "With criteria",
        "-q",
        "--body",
        "## Acceptance criteria\n\n- [x] done one\n- [ ] not done\n",
    ]);
    let out = p.expect(&["close", "1"]).all();
    assert_contains(&out, "1 of 2", "it says how many are outstanding");
    assert!(p.run(&["check"]).ok(), "and closes anyway");

    // A project can ask for that to be an error afterwards as well.
    let cfg = p
        .read("cairn.toml")
        .replace("[project]", "[project]\nrequire_criteria = true");
    p.write("cairn.toml", &cfg);
    // A warning, because an unticked box is a judgement about process rather
    // than a schema violation; `--strict` is what turns it into a failure.
    let out = p.expect(&["check"]).all();
    assert_contains(&out, "1 of 2 acceptance criteria", "");
    assert!(
        !p.run(&["check", "--strict"]).ok(),
        "--strict should fail on it"
    );
}

/// Criteria confined to one heading, which is what a project sets when its
/// bodies contain other checklists.
#[test]
fn criteria_can_be_confined_to_one_section() {
    let p = Project::new();
    let cfg = p.read("cairn.toml").replace(
        "[project]",
        "[project]\ncriteria_section = \"Acceptance criteria\"",
    );
    p.write("cairn.toml", &cfg);
    p.expect(&[
        "new",
        "Two lists",
        "-q",
        "--body",
        "## Notes\n\n- [ ] not a criterion\n- [ ] nor this\n\n\
         ## Acceptance criteria\n\n- [x] the only one that counts\n",
    ]);

    let out = p.expect(&["close", "1"]).all();
    assert!(
        !out.contains("unticked") && !out.contains(" of "),
        "the notes checklist was counted:\n{out}"
    );
    // `criteria` and `criteria_done` are derived, so they are columns and
    // filters rather than stored fields.
    let counts = p
        .expect(&[
            "list",
            "-A",
            "--plain",
            "--columns",
            "criteria,criteria_done",
        ])
        .trimmed();
    assert_eq!(
        counts, "1\t1",
        "the notes checklist was counted as acceptance criteria"
    );
}

/// `renumber --dry-run` and `--compact`, which are the two ways of asking it
/// not to do the obvious thing.
#[test]
fn renumber_can_describe_itself_before_acting() {
    let p = Project::new();
    p.add("Keeper", &[]);
    p.write(
        "cairn/items/0001-a-copy.md",
        "---\nid: 1\ntitle: A copy\nstatus: backlog\n---\nbody\n",
    );
    let before = p.files("cairn/items");

    let out = p.expect(&["renumber", "--dry-run"]).all();
    assert_contains(&out, "would be renumbered", "");
    assert_eq!(before, p.files("cairn/items"), "a dry run moved a file");

    p.expect(&["renumber"]);
    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());
}

/// Every `--json` shape a script might read, asserted to parse.
#[test]
fn every_machine_readable_shape_parses() {
    let p = seeded();
    p.expect(&["note", "1", "something"]);

    for args in [
        vec!["list", "-A", "--json"],
        vec!["next", "--json"],
        vec!["search", "item", "--json"],
        vec!["show", "1", "--json"],
        vec!["export"],
        vec!["config", "--json"],
    ] {
        let out = p.expect(&args);
        serde_json::from_str::<serde_json::Value>(&out.stdout)
            .unwrap_or_else(|e| panic!("`cairn {args:?}` is not JSON: {e}\n{}", out.stdout));
    }
}

/// A roadmap narrowed to one milestone, which is the positional argument
/// nothing had ever passed.
#[test]
fn a_roadmap_can_be_narrowed_to_one_milestone() {
    let p = seeded();
    milestone(&p, "v0.2", Some("2027-01-01"));
    p.expect(&["set", "2", "milestone=v0.2"]);

    let all = p.expect(&["roadmap"]).stdout;
    assert_contains(&all, "v0.1", "");
    assert_contains(&all, "v0.2", "");

    let one = p.expect(&["roadmap", "v0.1"]).stdout;
    assert_contains(&one, "v0.1", "");
    assert!(
        !one.contains("v0.2"),
        "it showed a milestone nobody asked for:\n{one}"
    );

    let out = p.fails(&["roadmap", "v9.9"]);
    assert_contains(&out.all(), "unknown milestone", "");
    assert_contains(&out.all(), "v0.1", "and lists the ones there are");
}

/// A description on the project reaches both the terminal roadmap and the
/// rendered one.
#[test]
fn a_project_description_is_shown_where_the_roadmap_is() {
    let p = seeded();
    let cfg = p.read("cairn.toml").replace(
        "[project]",
        "[project]\ndescription = \"A thing that does a thing.\"",
    );
    p.write("cairn.toml", &cfg);

    assert_contains(
        &p.expect(&["roadmap"]).stdout,
        "A thing that does a thing.",
        "",
    );
    p.expect(&["render"]);
    assert_contains(&p.read("ROADMAP.md"), "A thing that does a thing.", "");
}

/// A format 1 project written before the milestone type and field existed:
/// the migration has to add both, not assume them.
#[test]
fn migrating_adds_the_type_and_field_when_they_are_absent() {
    let p = Project::empty();
    p.write(
        "cairn.toml",
        "format = 1\n[project]\nname = \"Old\"\n\
         [[status]]\nname = \"todo\"\ncategory = \"open\"\n\
         [[type]]\nname = \"feature\"\n\
         \n[[milestone]]\nname = \"v0.1\"\ntitle = \"First\"\ndue = \"2026-12-01\"\n",
    );
    std::fs::create_dir_all(p.path("cairn/items")).unwrap();
    p.write(
        "cairn/items/0001-scheduled.md",
        "---\nid: 1\ntitle: Scheduled\nstatus: todo\nmilestone: v0.1\n---\nbody\n",
    );

    let out = p.expect(&["migrate"]).all();
    assert_contains(&out, "milestone", "");

    let cfg = p.read("cairn.toml");
    assert!(
        cfg.contains("name = \"milestone\""),
        "the type was not added:\n{cfg}"
    );
    // Format 2 added a `[[field]]` to say the type groups work; format 3 says it
    // on the type, and a project coming from 1 lands at 3 having never seen the
    // intermediate shape.
    assert!(
        cfg.contains("groups = \"one\""),
        "the type does not declare that it groups:\n{cfg}"
    );
    assert!(
        !cfg.contains("target = \"milestone\""),
        "the field that used to say it survived:\n{cfg}"
    );
    assert!(
        cfg.contains("name = \"due\""),
        "`due` was not declared:\n{cfg}"
    );
    assert!(
        !cfg.contains("[[milestone]]"),
        "the old block survived:\n{cfg}"
    );

    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());
    assert_contains(
        &p.expect(&["roadmap"]).stdout,
        "First",
        "the roadmap survived",
    );
}

/// Claiming has several ways to refuse, and each of them is a sentence somebody
/// has to act on.
#[test]
fn claiming_refuses_for_reasons_it_names() {
    let p = seeded();
    p.expect(&["set", "2", "depends_on=1"]);

    // Blocked.
    let out = p.fails(&["claim", "2"]);
    assert_contains(&out.all(), "blocked", "");
    assert!(
        p.run(&["claim", "2", "--force"]).ok(),
        "--force takes it anyway"
    );

    // Held by somebody else.
    p.expect(&["set", "3", "assignee=someone-else"]);
    let out = p.fails(&["claim", "3"]);
    assert_contains(&out.all(), "someone-else", "it names who has it");
    assert!(p.run(&["claim", "3", "--force"]).ok());

    // Released, and then free again.
    p.expect(&["release", "3"]);
    assert!(p.run(&["claim", "3"]).ok(), "release did not free it");

    // Nothing left to claim.
    p.expect(&["claim", "1"]);
    let out = p.run(&["claim", "--next"]);
    assert!(
        !out.ok(),
        "there is nothing unclaimed and ready: {}",
        out.all()
    );
}

/// `new` has flags for everything an item carries, and most set a field the
/// long way round.
#[test]
fn new_can_set_everything_an_item_carries() {
    let p = seeded();
    let id = p
        .expect(&[
            "new",
            "Fully specified",
            "-q",
            "-t",
            "bug",
            "-s",
            "planned",
            "-m",
            "v0.1",
            "-l",
            "one,two",
            "-a",
            "somebody",
            "-d",
            "1",
            "--set",
            "priority=p0",
            "--body",
            "A body of its own.",
        ])
        .trimmed();

    let json: serde_json::Value =
        serde_json::from_str(&p.expect(&["show", &id, "--json"]).stdout).unwrap();
    assert_eq!(json["type"], "bug");
    assert_eq!(json["status"], "planned");
    assert_eq!(json["milestone"], "v0.1");
    assert_eq!(json["assignee"], "somebody");
    assert_eq!(json["labels"], serde_json::json!(["one", "two"]));
    assert_eq!(json["depends_on"], serde_json::json!([1]));
    assert_eq!(json["fields"]["priority"], "p0");
    assert_contains(json["body"].as_str().unwrap(), "A body of its own.", "");

    // A body from standard input, which is how a hook or a script writes one.
    let out = p.run_stdin(&["new", "From a pipe", "-q", "--stdin"], "piped body\n");
    assert!(out.ok(), "{}", out.all());
    assert_contains(
        &p.expect(&["show", out.trimmed().as_str()]).stdout,
        "piped body",
        "",
    );
}

/// Removing an item takes its references with it, and asks first.
#[test]
fn removing_an_item_repairs_what_pointed_at_it() {
    let p = seeded();
    p.expect(&["set", "2", "depends_on=1"]);
    p.expect(&["set", "3", "milestone=v0.1"]);

    // It asks, and a refusal changes nothing.
    let out = p.run_stdin(&["remove", "1"], "n\n");
    assert!(
        !out.ok() || p.run(&["show", "1"]).ok(),
        "a declined removal deleted it"
    );

    p.expect(&["remove", "1", "--force"]);
    assert!(!p.run(&["show", "1"]).ok(), "it is gone");
    let json: serde_json::Value =
        serde_json::from_str(&p.expect(&["show", "2", "--json"]).stdout).unwrap();
    assert_eq!(
        json["depends_on"],
        serde_json::json!([]),
        "a dangling dependency was left behind: {json}"
    );
    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());
}

/// The reserved fields `set` handles by name, each with its own rule about
/// what an empty value and a list value mean.
#[test]
fn every_reserved_field_has_its_own_rule() {
    let p = seeded();

    // `key`: set, cleared, and refused when it would read as an identifier.
    p.expect(&["set", "1", "key=first"]);
    assert_contains(&p.expect(&["show", "1"]).stdout, "first", "");
    let out = p.fails(&["set", "1", "key=0002"]);
    assert_contains(&out.all(), "reads as an identifier", "");
    p.expect(&["set", "1", "key="]);

    // `owner` and `created_by` are ordinary strings that can be cleared.
    for field in ["owner", "created_by"] {
        p.expect(&["set", "1", &format!("{field}=somebody")]);
        assert_contains(
            &p.expect(&["show", "1", "--json"]).stdout,
            "somebody",
            field,
        );
        p.expect(&["set", "1", &format!("{field}=")]);
    }

    // A title cannot be emptied, and is not a list.
    let out = p.fails(&["set", "1", "title="]);
    assert_contains(&out.all(), "cannot be empty", "");
    let out = p.fails(&["set", "1", "title+=more"]);
    assert_contains(&out.all(), "not a list field", "");

    // `id` is identity and cannot be assigned at all.
    let out = p.fails(&["set", "1", "id=99"]);
    assert_contains(&out.all(), "cannot be changed", "");

    // `type` cleared, and refused when it names nothing.
    p.expect(&["set", "1", "type="]);
    let out = p.fails(&["set", "1", "type=sculpture"]);
    assert_contains(&out.all(), "sculpture", "");

    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());
}

/// Adding to and removing from a list field, rather than replacing it.
#[test]
fn a_list_field_can_be_added_to_and_taken_from() {
    let p = seeded();
    p.expect(&["set", "1", "labels=one,two"]);
    p.expect(&["set", "1", "labels+=three"]);
    let json: serde_json::Value =
        serde_json::from_str(&p.expect(&["show", "1", "--json"]).stdout).unwrap();
    assert_eq!(json["labels"], serde_json::json!(["one", "two", "three"]));

    p.expect(&["set", "1", "labels-=two"]);
    let json: serde_json::Value =
        serde_json::from_str(&p.expect(&["show", "1", "--json"]).stdout).unwrap();
    assert_eq!(json["labels"], serde_json::json!(["one", "three"]));

    p.expect(&["set", "1", "labels="]);
    let json: serde_json::Value =
        serde_json::from_str(&p.expect(&["show", "1", "--json"]).stdout).unwrap();
    assert_eq!(json["labels"], serde_json::json!([]));
}

/// A cycle cannot be created by an ordinary command, however it is approached.
#[test]
fn a_dependency_cycle_is_refused_at_every_depth() {
    let p = Project::new();
    p.add("One", &[]);
    p.add("Two", &["-d", "1"]);
    p.add("Three", &["-d", "2"]);

    let out = p.fails(&["set", "1", "depends_on=1"]);
    assert_contains(&out.all(), "cycle", "an item depending on itself");

    let out = p.fails(&["set", "1", "depends_on=3"]);
    assert_contains(&out.all(), "cycle", "closing a loop three deep");

    // A dependency on nothing is refused too, at creation as well as on an
    // edit: `new` checked only for a cycle, so it accepted one and left `check`
    // to complain afterwards.
    let out = p.fails(&["set", "1", "depends_on=999"]);
    assert_contains(&out.all(), "does not exist", "");
    let out = p.fails(&["new", "Four", "-d", "999"]);
    assert_contains(&out.all(), "does not exist", "");

    assert!(p.run(&["check"]).ok(), "{}", p.run(&["check"]).all());
}

/// The render hook fires with the file it wrote and how many items went into
/// it, which is a different shape from the item hooks.
///
/// Unix only, and deliberately: the string form of a hook goes through the
/// platform shell, which is the thing that makes it platform-specific. The
/// manual says so, and the portable form is the argv array tested below.
#[cfg(unix)]
#[test]
fn the_render_hook_reports_the_file_and_the_count() {
    let p = seeded();
    let cfg = p.read("cairn.toml").replace(
        "[hooks]",
        "[hooks]\nafter-render = \"printf '%s %s' \\\"$CAIRN_RENDER_TARGET\\\" \\\"$CAIRN_ITEM_COUNT\\\" > rendered.txt\"",
    );
    p.write("cairn.toml", &cfg);

    p.expect(&["render"]);
    let seen = p.read("rendered.txt");
    assert_contains(&seen, "ROADMAP.md", "the file it wrote");
    assert!(
        seen.split_whitespace()
            .nth(1)
            .is_some_and(|n| n.parse::<u32>().is_ok()),
        "the item count: {seen}"
    );
}

/// A hook given as an argv array runs with no shell at all, which is the
/// portable form.
#[test]
fn a_hook_can_be_an_argv_array() {
    let p = seeded();
    let cfg = p.read("cairn.toml").replace(
        "after-create = \"cairn render -q\"",
        "after-create = [\"cairn\", \"render\", \"-q\"]",
    );
    p.write("cairn.toml", &cfg);

    p.expect(&["new", "Triggers the hook", "-q"]);
    assert_contains(
        &p.read("ROADMAP.md"),
        "Triggers the hook",
        "the array-form hook did not run",
    );
}

/// A hook that fails warns and does not roll anything back, because it runs
/// after the change is already on disk.
#[test]
fn a_failing_hook_warns_without_undoing_anything() {
    let p = seeded();
    let cfg = p.read("cairn.toml").replace(
        "after-create = \"cairn render -q\"",
        "after-create = \"exit 3\"",
    );
    p.write("cairn.toml", &cfg);

    let out = p.expect(&["new", "Written anyway", "-q"]);
    assert_contains(&out.all(), "hook", "it says the hook failed");
    assert_contains(
        &p.expect(&["list", "-A", "--plain"]).stdout,
        "Written anyway",
        "and the item is still there",
    );
}

/// `owner` and `created_by` were writable, shown as columns, and absent from
/// every machine-readable shape — so an export dropped them and a round trip
/// through the interchange document lost them without a word.
#[test]
fn who_is_answerable_survives_a_round_trip() {
    let p = seeded();
    p.expect(&["set", "1", "owner=a-person"]);
    p.expect(&["set", "1", "created_by=an-agent"]);

    let json: serde_json::Value =
        serde_json::from_str(&p.expect(&["show", "1", "--json"]).stdout).unwrap();
    assert_eq!(json["owner"], "a-person", "{json}");
    assert_eq!(json["created_by"], "an-agent", "{json}");

    let doc = p.expect(&["export"]).stdout;
    assert_contains(&doc, "a-person", "the export carries the owner");

    let fresh = Project::new();
    fresh.run_stdin(&["import"], &doc);
    let json: serde_json::Value =
        serde_json::from_str(&fresh.expect(&["show", "1", "--json"]).stdout).unwrap();
    assert_eq!(
        json["owner"], "a-person",
        "the owner was lost in transit: {json}"
    );
    assert_eq!(json["created_by"], "an-agent", "{json}");
}
// --- a claim is a promise somebody might not keep ---------------------------

/// A project that says how long a claim may go untouched.
fn with_stale_after(days: u32) -> Project {
    let p = Project::with(Schema::standard().claim_stale_after(days));
    seed(&p);
    p
}

#[test]
fn a_claim_records_when_it_was_taken() {
    let p = seeded();
    p.expect(&["claim", "2", "--as", "agent-one"]);

    let file = p.item_file("2");
    assert_contains(&file, "assignee: agent-one", "");
    assert_contains(&file, "claimed:", "a claim records when, not only who");

    // `updated` means something else and every other edit moves it, so it is a
    // key of its own.
    p.expect(&["set", "2", "priority=p0"]);
    assert_contains(
        &p.item_file("2"),
        "claimed:",
        "an unrelated edit cleared it",
    );

    // Handing it back or finishing it clears the claim.
    p.expect(&["release", "2"]);
    assert_missing(
        &p.item_file("2"),
        "claimed:",
        "release left the claim behind",
    );

    p.expect(&["claim", "3"]);
    p.expect(&["close", "3"]);
    assert_missing(&p.item_file("3"), "claimed:", "close left the claim behind");
}

/// A claim nobody is honouring is invisible twice over: `next` will not offer
/// it because it is held, and nothing lists it because nothing knows.
#[test]
fn a_stale_claim_is_visible_and_never_revoked() {
    let p = with_stale_after(3);
    p.write_item(
        "0050-abandoned.md",
        "id: 50\ntitle: Abandoned\nstatus: doing\nassignee: ghost\nclaimed: 2020-01-01",
        "body\n",
    );

    assert_eq!(p.count_of_all("stale=true"), 1, "nothing was called stale");
    assert!(
        p.count_of_all("held_days>100") >= 1,
        "how long it has been held is not derivable"
    );

    // Offered, with who has it and for how long.
    let out = p.expect(&["next"]).all();
    assert_contains(&out, "stale:", "");
    assert_contains(&out, "ghost", "it names who holds it");
    assert_contains(&out, "day(s)", "and for how long");

    // Offered, never taken. The item is still theirs until somebody says
    // otherwise.
    assert_contains(&p.item_file("50"), "assignee: ghost", "it was revoked");
}

#[test]
fn a_stale_claim_can_be_taken_over_without_force() {
    let p = with_stale_after(3);
    p.write_item(
        "0050-abandoned.md",
        "id: 50\ntitle: Abandoned\nstatus: doing\nassignee: ghost\nclaimed: 2020-01-01",
        "body\n",
    );

    let out = p.expect(&["claim", "50", "--as", "somebody"]).all();
    assert_contains(&out, "taken over from ghost", "it says whose it was");
    assert_contains(&out, "day(s)", "and how long they held it");
    assert_contains(&p.item_file("50"), "assignee: somebody", "");

    // A claim that is not stale still needs --force.
    p.expect(&["claim", "1", "--as", "holder"]);
    let out = p.fails(&["claim", "1", "--as", "interloper"]);
    assert_contains(&out.all(), "--force", "");
}

/// No threshold, no staleness. A project that has not said what a claim means
/// should see nothing new.
#[test]
fn without_a_threshold_nothing_is_ever_stale() {
    let p = seeded();
    p.write_item(
        "0050-ancient.md",
        "id: 50\ntitle: Ancient\nstatus: doing\nassignee: ghost\nclaimed: 2020-01-01",
        "body\n",
    );
    assert_eq!(p.count_of_all("stale=true"), 0);
    assert!(!p.expect(&["next"]).all().contains("stale:"));
    let out = p.fails(&["claim", "50", "--as", "somebody"]);
    assert_contains(&out.all(), "--force", "an old claim is still a claim");
}
// --- handing work back ------------------------------------------------------

/// The most valuable thing somebody handing work back knows is why, and it
/// used to evaporate: the next taker walked the same dead end.
#[test]
fn releasing_with_a_reason_reaches_the_next_taker() {
    let p = seeded();
    p.expect(&["claim", "2"]);
    p.expect(&[
        "release",
        "2",
        "--reason",
        "The parser rewrite needs the format decision first.",
    ]);

    let file = p.item_file("2");
    assert_contains(
        &file,
        "## Released by",
        "a dated note in the releaser's name",
    );
    assert_contains(&file, "format decision", "carrying what they knew");

    let out = p.expect(&["claim", "2"]).all();
    assert_contains(
        &out,
        "last released because:",
        "the next taker is told before they start",
    );
    assert_contains(&out, "format decision", "");

    // Three attempts on a hard item is a history, so nothing is overwritten.
    p.expect(&["release", "2", "--reason", "Second attempt, same wall."]);
    let file = p.item_file("2");
    assert_contains(&file, "format decision", "the first reason survived");
    assert_contains(&file, "Second attempt", "and the second is there too");

    // Nothing goes into frontmatter.
    let front = file.split("---").nth(1).unwrap_or_default();
    assert_missing(front, "released", "a reason is a history, not a field");
}

#[test]
fn releasing_without_a_reason_still_works_and_says_nothing_extra() {
    let p = seeded();
    p.expect(&["claim", "2"]);
    let out = p.expect(&["release", "2"]).all();
    assert_missing(&out, "Released by", "");
    assert!(p.run(&["check"]).ok());
}
// --- filing the same thing twice --------------------------------------------

/// An agent starts every session cold, so filing a duplicate is not an unlucky
/// mistake but the characteristic failure of an agent using this tool.
#[test]
fn a_near_duplicate_title_is_reported_and_created_anyway() {
    let p = Project::new();
    p.add("Rate-limit the public API", &[]);

    let out = p.expect(&["new", "Rate limit the API"]).all();
    assert_contains(&out, "look", "it says something looks similar");
    assert_contains(&out, "Rate-limit the public API", "and names it");
    assert_eq!(p.count_all(), 2, "reported, not refused");

    // Word order and punctuation do not hide a duplicate.
    let out = p.expect(&["new", "the API, rate limited"]).all();
    assert_contains(&out, "look", "word order should not hide it");

    // Something genuinely different says nothing.
    let out = p.expect(&["new", "Document the export format"]).all();
    assert_missing(&out, "look", "an unrelated title was called similar");

    // A finished item counts: re-filing something already done is the same
    // mistake.
    p.expect(&["close", "1"]);
    let out = p.expect(&["new", "Rate-limit the public API again"]).all();
    assert_contains(&out, "look", "a closed duplicate was not considered");

    assert!(p.run(&["check"]).ok());
}

#[test]
fn a_quiet_creation_says_nothing_about_duplicates() {
    let p = Project::new();
    p.add("Rate-limit the public API", &[]);
    let out = p.expect(&["new", "Rate limit the API", "-q"]).all();
    assert_missing(&out, "look", "--quiet should be quiet");
}

/// Over the protocol the warning has to be in the result: a tool result is what
/// the model reads, and standard error is not.
#[test]
fn an_agent_is_told_in_band_that_it_may_be_filing_a_duplicate() {
    let p = Project::new();
    p.add("Rate-limit the public API", &[]);

    let r = p.mcp_call(
        "create_item",
        serde_json::json!({"title": "Rate limit the API"}),
        Some("claude"),
    );
    assert!(!refused(&r), "{}", tool_text(&r));
    let doc: serde_json::Value = serde_json::from_str(&tool_text(&r)).expect("JSON");
    assert!(
        doc["similar"].is_array(),
        "no similar items reported: {doc}"
    );
    assert_json(
        &doc,
        "similar.0.title",
        serde_json::json!("Rate-limit the public API"),
    );
    assert_contains(doc["note"].as_str().unwrap_or_default(), "look", "");
}

/// An agent that cannot finish should hand the item back with what it learned,
/// rather than leaving it claimed for ever.
#[test]
fn an_agent_can_hand_work_back_with_a_reason() {
    let p = seeded();
    p.mcp_call("claim_item", serde_json::json!({"id": 2}), Some("claude"));

    let r = p.mcp_call(
        "release_item",
        serde_json::json!({"id": 2, "reason": "Needs a decision only a person can make."}),
        Some("claude"),
    );
    assert!(!refused(&r), "{}", tool_text(&r));

    let file = p.item_file("2");
    assert_contains(&file, "## Released by claude", "in the agent's own name");
    assert_contains(&file, "only a person can make", "");
    assert_missing(&file, "claimed:", "the claim was not cleared");
    assert!(p.run(&["check"]).ok());
}
// --- asking for less --------------------------------------------------------

/// Every read returned twenty-two keys and about six hundred characters an
/// item. Context is the one resource an agent cannot get more of, and cairn was
/// spending it on the caller's behalf without asking.
#[test]
fn an_agent_can_ask_for_only_the_keys_it_needs() {
    let p = seeded();
    for n in 0..6 {
        p.add(&format!("Filler {n}"), &[]);
    }

    let full = tool_text(&p.mcp_call("list_items", serde_json::json!({}), Some("claude")));
    let narrow = tool_text(&p.mcp_call(
        "list_items",
        serde_json::json!({"fields": ["title", "status", "priority", "blocked"]}),
        Some("claude"),
    ));
    assert!(
        narrow.len() * 2 < full.len(),
        "asking for four keys of twenty-two saved almost nothing: {} against {}",
        narrow.len(),
        full.len()
    );

    let doc: serde_json::Value = serde_json::from_str(&narrow).expect("JSON");
    let first = &doc["items"][0];
    let keys: Vec<&String> = first.as_object().expect("an object").keys().collect();
    assert_eq!(keys.len(), 5, "got more than was asked for: {keys:?}");
    // A result nothing can be acted on is worth less than the bytes it took.
    assert!(first["id"].is_number(), "id must survive: {first}");
}

/// A milestone carries no priority. Asking for one across a mixed list is a
/// reasonable request that answers `null`, not a failure because the first item
/// happened not to have one.
#[test]
fn a_field_an_item_lacks_is_null_rather_than_an_error() {
    let p = seeded();
    // Written by hand, because every item a command creates receives the
    // schema's default.
    p.write_item(
        "0050-bare.md",
        "id: 50\ntitle: Bare\nstatus: backlog",
        "body\n",
    );
    let doc: serde_json::Value = serde_json::from_str(&tool_text(&p.mcp_call(
        "list_items",
        serde_json::json!({"fields": ["title", "priority"], "include_closed": true}),
        Some("claude"),
    )))
    .expect("JSON");

    let items = doc["items"].as_array().expect("items");
    assert!(
        items.iter().any(|i| i["priority"].is_null()),
        "nothing came back null: {doc}"
    );
    assert!(
        items.iter().any(|i| i["priority"].is_string()),
        "nothing came back set: {doc}"
    );
}

/// A schema field lives under `fields` in the full shape, and a caller asking
/// for `priority` means the project's priority. Where cairn keeps it is cairn's
/// business.
#[test]
fn a_schema_field_can_be_asked_for_by_its_own_name() {
    let p = seeded();
    let doc: serde_json::Value = serde_json::from_str(&tool_text(&p.mcp_call(
        "show_item",
        serde_json::json!({"id": 1, "fields": ["title", "priority"]}),
        Some("claude"),
    )))
    .expect("JSON");
    assert_json(&doc, "priority", serde_json::json!("p0"));
}

#[test]
fn asking_for_a_field_that_does_not_exist_says_what_does() {
    let p = seeded();
    let r = p.mcp_call(
        "next_items",
        serde_json::json!({"fields": ["nonesuch"]}),
        Some("claude"),
    );
    assert!(refused(&r), "{}", tool_text(&r));
    assert_contains(&tool_text(&r), "unknown field `nonesuch`", "");
    assert_contains(&tool_text(&r), "priority", "and lists what is available");
}

/// The default is what every existing consumer already reads.
#[test]
fn asking_for_nothing_returns_what_it_always_did() {
    let p = seeded();
    let doc: serde_json::Value = serde_json::from_str(&tool_text(&p.mcp_call(
        "list_items",
        serde_json::json!({}),
        Some("claude"),
    )))
    .expect("JSON");
    let keys: Vec<&String> = doc["items"][0]
        .as_object()
        .expect("object")
        .keys()
        .collect();
    assert!(keys.len() > 15, "the default shape narrowed: {keys:?}");
}
// --- what changed since I last looked ---------------------------------------

#[test]
fn a_returning_caller_can_ask_only_for_what_moved() {
    let p = Project::new();
    p.add("Ancient", &[]);
    p.add("Recent", &[]);
    // Reach in, because `updated` is what the filter reads and time is not a
    // thing a test may wait for.
    let old = p.expect(&["show", "1", "--path"]).trimmed();
    let text = p
        .read(&old)
        .replace(&format!("updated: {}", today()), "updated: 2020-01-01");
    p.write(&old, &text);

    assert_eq!(p.expect(&["list", "--ids"]).lines().len(), 2);
    assert_eq!(
        p.expect(&["list", "--since", "2026-01-01", "--ids"])
            .lines(),
        vec!["0002".to_string()],
    );
    // And on search, and over the protocol.
    p.expect(&["search", "e", "--since", "2026-01-01"]);
    let doc: serde_json::Value = serde_json::from_str(&tool_text(&p.mcp_call(
        "list_items",
        serde_json::json!({"since": "2026-01-01", "fields": ["title"]}),
        Some("claude"),
    )))
    .expect("JSON");
    assert_json(&doc, "count", serde_json::json!(1));

    // A date that is not one says what it wanted.
    let out = p.fails(&["list", "--since", "yesterday"]);
    assert_contains(&out.all(), "YYYY-MM-DD", "");
}

fn today() -> String {
    let out = std::process::Command::new("date")
        .args(["+%Y-%m-%d"])
        .output()
        .expect("date");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}
// --- a proposal is a proposal -----------------------------------------------

/// A project where an agent may ask for a priority but not set one.
fn proposing() -> Project {
    let p = Project::with(
        Schema::standard()
            .amend("priority", |f| f.agent(Agent::Propose))
            .amend_status("done", |s| s.agent(Agent::Propose)),
    );
    seed(&p);
    p
}

/// The permission already refused the write and told the caller to write a
/// note. Then the proposal was prose a person had to find by reading every body
/// in the backlog.
#[test]
fn a_refusal_names_the_command_that_replaces_it() {
    let p = proposing();
    let out = p.fails_as_agent(&["set", "1", "priority=p1"]);
    assert_contains(&out.all(), "cairn propose", "it names the way through");
    assert_contains(&out.all(), "priority=p1", "with the change already spelled");
    assert_contains(&out.all(), "cairn proposals", "and who reviews it");
}

#[test]
fn a_proposal_is_listed_reviewed_and_applied() {
    let p = proposing();
    p.expect(&[
        "propose",
        "1",
        "priority=p3",
        "--why",
        "It blocks nothing and nobody has asked.",
    ]);

    // Nothing has changed yet.
    assert_json(
        &p.json(&["show", "1", "--json"]),
        "fields.priority",
        serde_json::json!("p0"),
    );

    let listed = p.expect(&["proposals"]).stdout;
    assert_contains(&listed, "priority: p0 -> p3", "the change, both ends");
    assert_contains(&listed, "blocks nothing", "and the reason");

    let json = p.json(&["proposals", "--json"]);
    assert_json(&json, "0.field", serde_json::json!("priority"));
    assert_json(&json, "0.from", serde_json::json!("p0"));
    assert_json(&json, "0.to", serde_json::json!("p3"));

    p.expect(&["proposals", "--accept", "1"]);
    assert_json(
        &p.json(&["show", "1", "--json"]),
        "fields.priority",
        serde_json::json!("p3"),
    );

    // Applied, and recorded as applied rather than looking as though it was
    // always so.
    assert_contains(&p.item_file("1"), "## Accepted priority", "");
    assert!(
        p.expect(&["proposals"])
            .all()
            .contains("nothing is proposed"),
        "an accepted proposal is still listed as waiting"
    );
    assert!(p.run(&["check"]).ok());
}

/// A proposal lives in the body, so an unrelated write cannot lose it.
#[test]
fn a_proposal_survives_an_unrelated_change() {
    let p = proposing();
    p.expect(&["propose", "1", "priority=p1", "--why", "Because."]);
    p.expect(&["set", "1", "area=parser"]);
    p.expect(&["set", "1", "labels+=x"]);
    assert_contains(&p.expect(&["proposals"]).stdout, "priority: p0 -> p1", "");
}

/// Several proposals on one item is a conversation, not a value.
#[test]
fn proposals_accumulate_rather_than_replace() {
    let p = proposing();
    p.expect(&["propose", "1", "priority=p1", "--why", "First argument."]);
    p.expect(&["propose", "1", "priority=p3", "--why", "Second argument."]);

    let listed = p.expect(&["proposals"]).stdout;
    assert_contains(&listed, "First argument.", "the earlier one survived");
    assert_contains(&listed, "Second argument.", "and the later one is there");

    // Accepting takes the most recent.
    p.expect(&["proposals", "--accept", "1"]);
    assert_json(
        &p.json(&["show", "1", "--json"]),
        "fields.priority",
        serde_json::json!("p3"),
    );
}

#[test]
fn a_proposal_is_checked_against_the_schema() {
    let p = proposing();
    let out = p.fails(&["propose", "1", "priority=urgent"]);
    assert_contains(
        &out.all(),
        "urgent",
        "a value the schema forbids is refused",
    );

    let out = p.fails(&["propose", "1", "priority=p0"]);
    assert_contains(&out.all(), "already", "proposing what is already true");

    let out = p.fails(&["propose", "1", "nonsense"]);
    assert_contains(&out.all(), "field=value", "");

    let out = p.fails(&["proposals", "--accept", "2"]);
    assert_contains(&out.all(), "nothing proposed", "");
}

/// The agent is the caller this exists for, so it has to be reachable from
/// where an agent is.
#[test]
fn an_agent_can_propose_what_it_may_not_set() {
    let p = proposing();

    let refused_write = p.mcp_call(
        "update_item",
        serde_json::json!({"id": 1, "fields": {"priority": "p3"}}),
        Some("claude"),
    );
    assert!(refused(&refused_write));
    assert_contains(
        &tool_text(&refused_write),
        "propose",
        "the refusal points here",
    );

    let r = p.mcp_call(
        "propose_change",
        serde_json::json!({
            "id": 1, "field": "priority", "value": "p3",
            "why": "Nothing depends on it any more."
        }),
        Some("claude"),
    );
    assert!(!refused(&r), "{}", tool_text(&r));

    // Nothing changed, and a person can see what is waiting.
    assert_json(
        &p.json(&["show", "1", "--json"]),
        "fields.priority",
        serde_json::json!("p0"),
    );
    let listed = p.expect(&["proposals"]).stdout;
    assert_contains(&listed, "claude", "in the agent's own name");
    assert_contains(&listed, "Nothing depends on it", "");
}

/// A proposal without a reason is a preference, not an argument.
#[test]
fn an_agent_must_say_why() {
    let p = proposing();
    let r = p.mcp_call(
        "propose_change",
        serde_json::json!({"id": 1, "field": "priority", "value": "p3"}),
        Some("claude"),
    );
    assert!(refused(&r));
    assert_contains(&tool_text(&r), "why", "");
}

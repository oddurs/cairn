// cairn — the promises in the Stability chapter, asserted.
//
// Copyright (c) 2026 Oddur Sigurdsson. MIT licensed; see LICENSE.
//
// A promise nothing checks is a wish. Every field named in the manual as stable
// is listed here, so removing or renaming one fails the build rather than
// somebody's script — six months later, quietly, in a way they cannot tell was
// deliberate.
//
// Adding to these lists is how a field becomes promised, and should be a
// decision. Removing from them is a major release.
use std::collections::BTreeSet;
mod support;
use support::*;

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
    "propose_change",
    "release_item",
    "close_item",
    "add_note",
    "check",
];

/// A project with a dependency in it, so every shape that carries one is
/// exercised.
fn stable() -> Project {
    let p = Project::new();
    p.expect(&["new", "First", "-q"]);
    p.expect(&["new", "Second", "-q"]);
    p.expect(&["set", "2", "depends_on+=1", "-q"]);
    p
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
    let p = stable();

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
    let p = stable();
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
    let p = stable();
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
    let p = stable();
    let present: BTreeSet<String> = p
        .mcp_tools()
        .iter()
        .filter_map(|t| t["name"].as_str().map(str::to_string))
        .collect();

    assert_has(&present, MCP_TOOLS, "the MCP server");
}

/// 0 did it, 1 failed or found problems, 2 the command line was wrong. Nothing
/// else, ever — a script that branches on the status has to be able to.
#[test]
fn exit_codes_mean_what_the_manual_says() {
    let p = stable();

    assert_eq!(p.run(&["check"]).code, 0, "a valid project checks clean");
    assert_eq!(p.run(&["list"]).code, 0, "listing succeeds");

    // 2 is clap's, and is only ever clap's.
    assert_eq!(
        p.run(&["list", "--no-such-flag"]).code,
        2,
        "a bad command line is 2"
    );
    assert_eq!(
        p.run(&["no-such-command"]).code,
        2,
        "an unknown command is 2"
    );

    // 1 is the operation failing, or finding what it was asked to look for.
    assert_eq!(
        p.run(&["show", "999"]).code,
        1,
        "an item that does not exist is 1, not 2: the command line was fine"
    );
    std::fs::write(p.root().join("cairn/items/broken.md"), "not an item at all")
        .expect("writing a broken item");
    assert_eq!(p.run(&["check"]).code, 1, "a project with problems is 1");
}

/// `--ids` puts one identifier on a line and nothing else. Anything more
/// breaks every `| xargs` in existence.
#[test]
fn ids_output_is_only_ids() {
    let p = stable();
    for args in [
        vec!["list", "--ids"],
        vec!["next", "--ids"],
        vec!["search", "First", "--ids"],
    ] {
        for line in p.expect(&args).lines() {
            assert!(
                !line.trim().is_empty() && line.trim().chars().all(char::is_alphanumeric),
                "`cairn {args:?}` put something other than an identifier on a line: {line:?}"
            );
        }
    }
}

/// `--count` prints one integer and nothing else.
#[test]
fn count_output_is_only_a_number() {
    let p = stable();
    for args in [
        vec!["list", "--count"],
        vec!["next", "--count"],
        vec!["search", "First", "--count"],
    ] {
        let out = p.expect(&args).stdout;
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
    let p = stable();
    p.expect(&["set", "1", "status=doing", "-q"]);

    for args in [
        vec!["list", "--plain", "-A"],
        vec!["next", "--plain"],
        vec!["search", "First", "--plain"],
    ] {
        let out = p.expect(&args).stdout;
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

// --- the filter grammar -----------------------------------------------------

/// Operators the Stability chapter promises. An expression that works today
/// works in every later release of this major version.
const FILTER_OPERATORS: &[&str] = &["=", "!=", "~", "!~", ">", ">=", "<", "<="];

/// The pseudo-fields the Stability chapter names, exactly as it names them.
///
/// `filter::DERIVED_KEYS` is longer, and the difference is deliberate rather
/// than an oversight: the chapter promises four, and the test below fails if
/// the list grows without somebody deciding which side of the line the new one
/// falls on.
const PROMISED_PSEUDO_FIELDS: &[&str] = &["category", "blocked", "ready", "blockers"];

/// Every operator still parses and still means something.
///
/// This was the one promise in the Stability chapter with nothing holding it to
/// account, which matters more here than elsewhere: the grammar is what a saved
/// view in `cairn.toml` is written in, and `cairn.toml` is read by programs that
/// are not cairn.
#[test]
fn every_promised_filter_operator_still_works() {
    let p = stable();
    for op in FILTER_OPERATORS {
        let expr = format!("id{op}1");
        let out = p.run(&["list", "-A", "--filter", &expr, "--count"]);
        assert!(
            out.ok(),
            "`cairn list --filter '{expr}'` was refused, and the Stability \
             chapter promises this operator:\n{}",
            out.all()
        );
        assert!(
            out.stdout.trim().parse::<usize>().is_ok(),
            "`--count` did not print a number for `{expr}`: {}",
            out.stdout
        );
    }

    // An empty value tests for absence, which the Filters chapter documents and
    // `cairn.toml`'s own `triage` view depends on.
    for expr in ["milestone=", "milestone!="] {
        assert!(
            p.run(&["list", "-A", "--filter", expr, "--count"]).ok(),
            "`{expr}` was refused"
        );
    }

    // Alternatives within a clause, and clauses combined with AND.
    assert!(
        p.run(&[
            "list",
            "-A",
            "--filter",
            "status=backlog|planned,id>0",
            "--count"
        ])
        .ok()
    );
}

/// A promised pseudo-field resolves, and is not reported as a typo.
///
/// The second half is what ties the promise to the diagnostic: `list` now warns
/// about a filter naming a field the schema does not declare, and a pseudo-field
/// the manual promises must never be what it warns about.
#[test]
fn a_promised_pseudo_field_is_never_called_a_typo() {
    let p = stable();
    for field in PROMISED_PSEUDO_FIELDS {
        let expr = format!("{field}!=");
        let out = p.run(&["list", "-A", "--filter", &expr, "--count"]);
        assert!(out.ok(), "`{expr}` was refused:\n{}", out.all());
        assert_missing(
            &out.all(),
            "not a declared field",
            "a pseudo-field the Stability chapter promises",
        );
    }

    // And `cairn check` agrees, which is where the same list used to live alone.
    let p2 = Project::new();
    let cfg = p2.read("cairn.toml").replace(
        "[hooks]",
        "[[view]]\nname = \"promised\"\nfilter = \"category=active,blocked=false\"\n\n[hooks]",
    );
    p2.write("cairn.toml", &cfg);
    assert_missing(
        &p2.expect(&["check"]).all(),
        "not a declared field",
        "`check` called a promised pseudo-field undeclared",
    );
}

/// The chapter enumerates four pseudo-fields. `DERIVED_KEYS` has more, and the
/// manual recommends scripting with some of them — `criteria_met` is how the
/// Acceptance criteria chapter suggests building a gate.
///
/// This does not decide which are promised. It fails when the set of derived
/// keys changes, so that the question is answered on purpose rather than by
/// precedent, which is the whole reason the Stability chapter exists.
#[test]
fn the_derived_keys_are_the_ones_the_manual_accounts_for() {
    // Every name `cairn` resolves without a `[[field]]` declaring it.
    const DERIVED: &[&str] = &[
        "category",
        "closed",
        "done",
        "blocked",
        "ready",
        "blockers",
        "contains",
        "descendants",
        "depth",
        "leaf",
        "progress",
        "criteria",
        "criteria_done",
        "criteria_met",
        "stale",
        "held_days",
    ];

    let p = stable();
    for key in DERIVED {
        let expr = format!("{key}!=");
        let out = p.run(&["list", "-A", "--filter", &expr, "--count"]);
        assert!(
            out.ok(),
            "`{expr}` was refused, so this list and `filter::DERIVED_KEYS` \
             disagree:\n{}",
            out.all()
        );
        assert_missing(
            &out.all(),
            "not a declared field",
            "a derived key reported as undeclared",
        );
    }

    // The manual documents every one of them in the Filters chapter, whether or
    // not the Stability chapter promises it. A key nobody documented is one
    // people find by reading the source.
    let manual = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("doc/cairn.texi"),
    )
    .expect("the manual");
    for key in DERIVED {
        // Either spelling the manual uses: inline, or as the entry of a
        // `@table @code`, where the braces are the table's job.
        let documented = manual.contains(&format!("@code{{{key}}}"))
            || manual.lines().any(|l| l.trim() == format!("@item {key}"));
        assert!(
            documented,
            "`{key}` resolves in a filter and the manual never mentions it"
        );
    }
}

// The selected queue is an explicit saved view, not a magic status name.
mod support;
use serde_json::json;
use support::*;

fn queue() -> Project {
    let p = Project::new();
    p.write(
        "cairn.toml",
        &Schema::bare()
            .id_width(4)
            .item_type(ItemType::new("feature"))
            .item_type(ItemType::new("bug"))
            .statuses(vec![
                Status::new("idea", Category::Open),
                Status::new("selected", Category::Open),
                Status::new("waiting", Category::Open),
                Status::new("working", Category::Active),
                Status::new("finished", Category::Done),
            ])
            .view("approved", "status=selected|working")
            .to_toml(),
    );
    p.add("Untriaged idea", &[]);
    p.add("External wait", &["-s", "waiting"]);
    p.add("Approved but blocked", &["-s", "selected", "-d", &p.id(1)]);
    p.add("Approved feature", &["-s", "selected"]);
    p.add("Approved bug", &["-s", "selected", "-t", "bug"]);
    p
}

fn payload(reply: serde_json::Value) -> serde_json::Value {
    assert_eq!(reply["isError"], false, "{reply}");
    serde_json::from_str(reply["content"][0]["text"].as_str().unwrap()).unwrap()
}

#[test]
fn every_selection_surface_uses_the_same_explicit_view() {
    let p = queue();
    let mut ready = [p.id(4), p.id(5)];
    ready.sort();
    let references: Vec<String> = ready
        .iter()
        .map(|id| {
            p.json(&["show", id, "--json"])["ref"]
                .as_str()
                .unwrap()
                .into()
        })
        .collect();
    assert_eq!(
        p.expect(&["next", "--view", "approved", "--ids"]).lines(),
        references
    );
    let reply = payload(p.mcp_call("next_items", json!({"view":"approved"}), None));
    assert_eq!(reply["count"], 2);
    assert_eq!(reply["items"][0]["id"], ready[0]);
    assert_eq!(reply["items"][1]["id"], ready[1]);
    assert_eq!(
        p.expect(&["claim", "--next", "--view", "approved", "-q"])
            .trimmed(),
        references[0]
    );
    let claimed = payload(p.mcp_call("claim_item", json!({"view":"approved"}), Some("second")));
    assert_eq!(claimed["id"], ready[1]);
    assert_eq!(claimed["status"], "working");
    let block = p.expect(&["agent", "--view", "approved"]).stdout;
    assert_contains(&block, "cairn next --view approved", "selected next");
    assert_contains(
        &block,
        "cairn claim --next --view approved",
        "atomic selection",
    );
    assert_contains(&block, "\"view\":\"approved\"", "MCP selection");
    p.expect(&["check", "--strict"]);
}

#[test]
fn caller_filters_only_narrow_a_view_and_bare_commands_are_unchanged() {
    let p = queue();
    let mut expected = [
        p.reference(1),
        p.reference(2),
        p.reference(4),
        p.reference(5),
    ];
    expected.sort();
    assert_eq!(p.expect(&["next", "--ids"]).lines(), expected);
    assert_eq!(
        p.expect(&[
            "next", "--view", "approved", "--filter", "type=bug", "--ids"
        ])
        .lines(),
        [p.reference(5)]
    );
    assert!(
        p.expect(&[
            "next",
            "--view",
            "approved",
            "--filter",
            "status=idea",
            "--ids"
        ])
        .lines()
        .is_empty()
    );
    let reply = payload(p.mcp_call(
        "next_items",
        json!({"view":"approved", "filter":"status=idea"}),
        None,
    ));
    assert_eq!(reply["count"], 0);
    let claim = p.mcp_call(
        "claim_item",
        json!({"view":"approved", "filter":"status=idea"}),
        None,
    );
    assert_eq!(claim["isError"], true);
    p.fails(&[
        "claim",
        "--next",
        "--view",
        "approved",
        "--filter",
        "status=idea",
    ]);
    assert_eq!(
        p.expect(&[
            "claim", "--next", "--view", "approved", "--filter", "type=bug", "-q"
        ])
        .trimmed(),
        p.reference(5)
    );
    let bare_next = p
        .expect(&["next", "--filter", "assignee=", "-n", "1", "--ids"])
        .trimmed();
    assert_eq!(p.expect(&["claim", "--next", "-q"]).trimmed(), bare_next);
}

#[test]
fn an_unknown_view_never_falls_back_to_the_whole_backlog() {
    let p = queue();
    for args in [
        vec!["next", "--view", "typo"],
        vec!["claim", "--next", "--view", "typo"],
        vec!["agent", "--view", "typo", "--write", "AGENTS.md"],
    ] {
        assert_contains(&p.fails(&args).all(), "unknown view", "fail closed");
    }
    for tool in ["next_items", "claim_item"] {
        assert_eq!(
            p.mcp_call(tool, json!({"view":"typo"}), None)["isError"],
            true
        );
    }
    assert!(!p.path("AGENTS.md").exists());
    assert_eq!(p.count_of("assignee!="), 0);
}

#[test]
fn simultaneous_claimers_take_distinct_approved_work() {
    let p = queue();
    let mut ids = std::thread::scope(|scope| {
        let workers: Vec<_> = ["one", "two"]
            .into_iter()
            .map(|who| {
                let p = &p;
                scope.spawn(move || {
                    p.expect(&["claim", "--next", "--view", "approved", "--as", who, "-q"])
                        .trimmed()
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|w| w.join().unwrap())
            .collect::<Vec<_>>()
    });
    ids.sort();
    let mut expected = [p.reference(4), p.reference(5)];
    expected.sort();
    assert_eq!(ids, expected);
}

#[test]
fn explicit_assignments_cannot_silently_ignore_a_view() {
    let p = queue();
    p.fails(&["claim", &p.id(1), "--next", "--view", "approved"]);
    p.fails(&["claim", &p.id(1), "--view", "approved"]);
    assert_eq!(
        p.mcp_call("claim_item", json!({"id":p.id(1),"view":"approved"}), None)["isError"],
        true
    );
    p.expect(&["claim", &p.id(1), "-q"]);
}

#[test]
fn regeneration_is_explicit_and_quotes_nontrivial_view_names() {
    let p = queue();
    p.append(
        "cairn.toml",
        "\n[[view]]\nname = \"team queue\"\nfilter = \"status=selected\"\n",
    );
    let block = p
        .expect(&["agent", "--view", "team queue", "--write", "AGENTS.md"])
        .all();
    assert_contains(&block, "wrote", "writes");
    assert_contains(
        &p.read("AGENTS.md"),
        "next --view 'team queue'",
        "shell argument",
    );
    p.expect(&["agent", "--write", "AGENTS.md"]);
    assert_missing(
        &p.read("AGENTS.md"),
        "next --view",
        "omitting the view deliberately restores bare selection",
    );
}

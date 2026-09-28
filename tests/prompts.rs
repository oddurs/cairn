//! Items as prompts: what a finished item concluded, and the work that quotes it.
mod support;
use serde_json::json;
use support::*;

fn result(p: &Project, id: &str) -> serde_json::Value {
    p.json(&["show", id, "--json"])["result"].clone()
}

#[test]
fn a_result_is_written_once_and_replaced_rather_than_repeated() {
    let p = Project::new();
    p.add("Find out", &["--body", "## Problem\n\nNobody knows.\n"]);
    assert_eq!(result(&p, "1"), json!(null), "none yet");

    p.expect(&["close", "1", "--result", "It is the cache."]);
    assert_eq!(result(&p, "1"), json!("It is the cache."));
    p.expect(&["note", "1", "Later: confirmed on the second machine."]);

    p.expect(&["close", "1", "--result", "It is the cache, twice over."]);
    let path = p.json(&["show", "1", "--json"])["path"].clone();
    let file = p.read(path.as_str().unwrap());
    assert_eq!(file.matches("## Result").count(), 1, "{file}");
    assert_eq!(result(&p, "1"), json!("It is the cache, twice over."));
    assert_contains(
        &file,
        "confirmed on the second machine",
        "a later note survives",
    );
}

#[test]
fn a_result_ends_at_the_next_heading_and_ignores_code() {
    let p = Project::new();
    p.add(
        "Shaped by hand",
        &[
            "--body",
            "## Result\n\nThe answer.\n\n```sh\n# not a heading\ncairn check\n```\n\n### Detail\n\nStill the result.\n\n## 2026-09-27\n\nA note.\n",
        ],
    );
    assert_eq!(
        result(&p, "1"),
        json!(
            "The answer.\n\n```sh\n# not a heading\ncairn check\n```\n\n### Detail\n\nStill the result."
        )
    );
}

#[test]
fn closing_without_a_result_names_the_work_left_with_nothing_to_quote() {
    let p = Project::new();
    p.add("Upstream", &[]);
    p.add("Downstream", &["-d", "1"]);
    p.add("Also downstream", &["-d", "1"]);

    let out = p.expect(&["close", "1"]);
    assert_contains(&out.stderr, "hint:", "");
    assert_contains(
        &out.stderr,
        "0002, 0003 depend(s) on 0001",
        "who is left without",
    );
    assert_contains(&out.stderr, "--result", "and how to fix it");

    p.expect(&["reopen", "1"]);
    let out = p.expect(&["close", "1", "--result", "Done: use the new API."]);
    assert_missing(&out.stderr, "hint:", "a result was given");

    let lone = p.add("Nothing waits on this", &[]);
    assert_missing(
        &p.expect(&["close", &lone]).stderr,
        "hint:",
        "nothing depends on it",
    );
}

#[test]
fn a_result_is_one_items_answer() {
    let p = Project::new();
    p.add("One", &[]);
    p.add("Two", &[]);
    assert_contains(
        &p.fails(&["close", "1", "2", "--result", "Both at once"])
            .all(),
        "one item's answer",
        "",
    );
}

#[test]
fn an_agent_closes_with_a_result_and_hears_who_is_waiting() {
    let p = Project::new();
    p.add("Upstream", &[]);
    p.add("Downstream", &["-d", "1"]);
    let reply = p.mcp_call(
        "close_item",
        json!({"id": 1, "result": "The schema is settled."}),
        Some("agent"),
    );
    assert_eq!(reply["isError"], false, "{reply}");
    let body: serde_json::Value =
        serde_json::from_str(reply["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(body["result"], "The schema is settled.");
    assert!(body.get("hint").is_none(), "{body}");
    assert_eq!(result(&p, "1"), json!("The schema is settled."));

    p.add("A second upstream", &[]);
    p.expect(&["set", "2", "depends_on+=3"]);
    let reply = p.mcp_call("close_item", json!({"id": 3}), Some("agent"));
    let body: serde_json::Value =
        serde_json::from_str(reply["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(body["result"], json!(null));
    assert!(body["hint"].as_str().unwrap().contains("0002"), "{body}");
}

/// A hand-written `# Result` sits above notes written as `## <date>`: those
/// are notes, and replacing the result must never erase one.
#[test]
fn replacing_a_result_never_erases_a_note_after_it() {
    let p = Project::new();
    p.add("Hand-written", &["--body", "# Result\n\nFirst answer.\n"]);
    p.expect(&["note", "1", "Something learned afterwards."]);
    p.expect(&["close", "1", "--result", "Second answer."]);
    let body = p.json(&["show", "1", "--json"])["body"].clone();
    let body = body.as_str().unwrap();
    assert_contains(body, "Something learned afterwards.", "the note survives");
    assert_missing(body, "First answer.", "the old result is gone");
    assert_eq!(result(&p, "1"), json!("Second answer."));
}

/// A result may have headings of its own. Written as they came, the first one
/// would end the section: readers would see half of it, and the next
/// replacement would leave the other half behind.
#[test]
fn a_result_with_its_own_headings_is_read_and_replaced_whole() {
    let p = Project::new();
    p.add("Findings", &[]);
    p.expect(&[
        "close",
        "1",
        "--result",
        "Found it.\n\n## Follow-ups\n\n- more",
    ]);
    assert_eq!(
        result(&p, "1"),
        json!("Found it.\n\n### Follow-ups\n\n- more"),
        "its heading sits below the Result heading"
    );
    p.expect(&["close", "1", "--result", "Replaced."]);
    let body = p.json(&["show", "1", "--json"])["body"].clone();
    assert_missing(
        body.as_str().unwrap(),
        "Follow-ups",
        "nothing is left behind",
    );
}

/// A code fence nobody closed must not hide the Result written after it, or
/// every close would add another one that nothing can find.
#[test]
fn an_unclosed_fence_does_not_hide_the_result() {
    let p = Project::new();
    p.add(
        "Stray fence",
        &["--body", "## Problem\n\n```\nforgot to close this\n"],
    );
    p.expect(&["close", "1", "--result", "Found anyway."]);
    assert_eq!(result(&p, "1"), json!("Found anyway."));
    p.expect(&["close", "1", "--result", "Again."]);
    let body = p.json(&["show", "1", "--json"])["body"].clone();
    assert_eq!(
        body.as_str().unwrap().matches("Result").count(),
        1,
        "{body}"
    );
}

/// A checkbox in a result is part of the answer, not a criterion the item still
/// asks for — or closing with one would leave the item unfinished by its own
/// account.
#[test]
fn a_checkbox_in_a_result_is_not_a_criterion() {
    let p = Project::new();
    p.add("Checked", &["--body=- [x] The one thing\n"]);
    p.expect(&[
        "close",
        "1",
        "--result",
        "Done.\n\n- [ ] migrate the rest later",
    ]);
    let criteria = p.json(&["show", "1", "--criteria", "--json"]);
    assert_eq!(criteria["total"], 1, "only the item's own: {criteria}");
    assert_missing(&p.expect(&["close", "1"]).stderr, "unticked", "");
}

/// A dropped item concluded nothing to quote, and the command the hint
/// suggests would move it to done.
#[test]
fn dropping_work_that_others_wait_on_is_not_told_to_record_a_result() {
    let p = Project::new();
    p.add("Abandoned", &[]);
    p.add("Waits", &["-d", "1"]);
    let out = p.expect(&["close", "1", "--status", "dropped"]);
    assert_missing(&out.stderr, "hint:", "");
}

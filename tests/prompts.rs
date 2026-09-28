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

// --- cairn prompt --------------------------------------------------------------

/// A milestone, a finished dependency with a result, one without, one still
/// open, and an item that rests on all of them.
fn stack() -> Project {
    let p = Project::new();
    p.add(
        "Items as prompts",
        &[
            "-t",
            "milestone",
            "--set",
            "key=prompts",
            "--body",
            "Cairn treats an item as a prompt.\n\nSecond paragraph.",
        ],
    );
    p.add("Decided", &["-m", "prompts"]);
    p.expect(&[
        "close",
        "2",
        "--result",
        "Views and conventions, never a runner.",
    ]);
    p.add("Quietly finished", &["-m", "prompts"]);
    p.expect(&["note", "3", "Done without saying much."]);
    p.expect(&["close", "3"]);
    p.add("Still going", &["-m", "prompts"]);
    p.add(
        "Compile the stack",
        &[
            "-m", "prompts", "-d", "2", "-d", "3", "-d", "4",
            "--body",
            "## Problem\n\nAgents assemble context by hand.\n\n## Approach\n\nAssemble it for them.\n\n## Acceptance criteria\n\n- [x] It prints\n- [ ] It is ordered\n- [ ] It is checked\n",
        ],
    );
    p.expect(&["note", "5", "Tried a template engine; too much."]);
    p
}

fn at(out: &str, needle: &str) -> usize {
    out.find(needle)
        .unwrap_or_else(|| panic!("`{needle}` not in:\n{out}"))
}

#[test]
fn a_prompt_reads_in_order_and_names_where_each_part_came_from() {
    let p = stack();
    let out = p.expect(&["prompt", "5"]).stdout;
    let order = [
        "# 0005 Compile the stack",
        "## How this project works",
        "## The outcome",
        "## What this builds on",
        "## The task",
        "## Done when",
        "## What earlier runs learned",
        "## When you stop",
    ];
    for pair in order.windows(2) {
        assert!(
            at(&out, pair[0]) < at(&out, pair[1]),
            "{} before {}",
            pair[0],
            pair[1]
        );
    }
    assert_contains(
        &out,
        "_From milestone prompts._",
        "the outcome names its source",
    );
    assert_contains(
        &out,
        "Cairn treats an item as a prompt.",
        "the milestone's first paragraph",
    );
    assert_missing(&out, "Second paragraph.", "and only that");
    assert_contains(&out, "_From depends_on: 0002, 0003, 0004._", "");
}

#[test]
fn a_dependency_contributes_its_result_else_its_last_note_else_its_blocking() {
    let p = stack();
    let out = p.expect(&["prompt", "5"]).stdout;
    assert_contains(
        &out,
        "Result: Views and conventions, never a runner.",
        "a result",
    );
    assert_contains(&out, "Its last note, ", "no result: the last note");
    assert_contains(&out, "Done without saying much.", "");
    assert_contains(
        &out,
        "0004 — Still going (backlog)\nNot finished",
        "an open one blocks",
    );
}

#[test]
fn done_when_numbers_criteria_as_tick_does() {
    let p = stack();
    let out = p.expect(&["prompt", "5"]).stdout;
    assert_contains(
        &out,
        "2. It is ordered\n3. It is checked",
        "open ones, numbered",
    );
    assert_contains(&out, "Already true:\n- It prints", "");
    p.expect(&["tick", "5", "3"]);
    let out = p.expect(&["prompt", "5"]).stdout;
    assert_contains(&out, "2. It is ordered", "");
    assert_missing(&out, "3. It is checked", "ticked by that number");
}

#[test]
fn the_task_leaves_out_what_other_layers_carry() {
    let p = stack();
    let out = p.expect(&["prompt", "5"]).stdout;
    let task = &out[at(&out, "## The task")..at(&out, "## Done when")];
    assert_contains(
        task,
        "### Problem",
        "the item's headings sit under the layer's",
    );
    assert_contains(task, "Assemble it for them.", "");
    assert_missing(task, "- [ ]", "criteria have their own layer");
    assert_missing(task, "template engine", "notes have their own layer");
    let learned = &out[at(&out, "## What earlier runs learned")..at(&out, "## When you stop")];
    assert_contains(learned, "### 20", "a note keeps its date, a level down");
}

#[test]
fn json_and_mcp_carry_the_same_prompt() {
    let p = stack();
    let doc = p.json(&["prompt", "5", "--json"]);
    let names: Vec<&str> = doc["layers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "How this project works",
            "The outcome",
            "What this builds on",
            "The task",
            "Done when",
            "What earlier runs learned",
            "When you stop"
        ]
    );
    let reply = p.mcp_call("prompt_item", json!({"id": 5}), Some("agent"));
    assert_eq!(reply["isError"], false, "{reply}");
    assert_eq!(
        reply["content"][0]["text"].as_str().unwrap(),
        p.expect(&["prompt", "5"]).stdout
    );
}

#[test]
fn a_bare_item_still_compiles() {
    let p = Project::new();
    p.add("Alone", &[]);
    let out = p.expect(&["prompt", "1"]).stdout;
    assert_contains(&out, "## The task", "");
    assert_missing(&out, "## The outcome", "nothing it is filed under");
    assert_missing(&out, "## What this builds on", "nothing it depends on");
    p.fails(&["prompt", "99"]);
}

#[test]
fn the_agent_instructions_start_from_the_prompt() {
    let p = Project::new();
    assert_contains(&p.expect(&["agent"]).stdout, "`cairn prompt <ID>`", "");
}

/// With a criteria section named, a checklist elsewhere is part of the task,
/// not a criterion, and must appear somewhere.
#[test]
fn a_checklist_outside_the_criteria_section_stays_in_the_task() {
    let p = Project::with(Schema::standard().criteria_section("Done when"));
    p.add(
        "Steps and criteria",
        &[
            "--body",
            "## Approach\n\n- [ ] migrate the table\n\n## Done when\n\n- [ ] it is migrated\n",
        ],
    );
    let out = p.expect(&["prompt", "1"]).stdout;
    let task = &out[at(&out, "## The task")..at(&out, "## Done when")];
    assert_contains(task, "- [ ] migrate the table", "a step, not a criterion");
    assert_contains(&out, "1. it is migrated", "");
}

#[test]
fn a_prompt_takes_a_key_as_show_does() {
    let p = stack();
    let out = p.expect(&["prompt", "prompts"]).stdout;
    assert_contains(&out, "# 0001 Items as prompts", "");
}

/// A body under one `# Context` still has notes after it.
#[test]
fn notes_under_a_top_level_heading_are_still_notes() {
    let p = Project::new();
    p.add(
        "Deep",
        &["--body", "# Context\n\n## Problem\n\nSomething.\n"],
    );
    p.expect(&["note", "1", "What the first run found."]);
    let out = p.expect(&["prompt", "1"]).stdout;
    let task = &out[at(&out, "## The task")..at(&out, "## What earlier runs learned")];
    assert_missing(task, "What the first run found.", "not part of the task");
    assert_contains(&out, "What the first run found.", "but in the prompt");
}

/// Prose beside the boxes in the criteria section is kept; boxes alone are not
/// repeated.
#[test]
fn prose_in_the_criteria_section_is_kept() {
    let p = Project::new();
    p.add(
        "Verified how",
        &[
            "--body",
            "## Acceptance criteria\n\n- [ ] it works\n\nVerified by running `make durability`.\n",
        ],
    );
    let out = p.expect(&["prompt", "1"]).stdout;
    let task = &out[at(&out, "## The task")..at(&out, "## Done when")];
    assert_contains(task, "Verified by running `make durability`.", "");
    assert_missing(task, "- [ ] it works", "the box is under Done when");
}

// --- prompt checks -------------------------------------------------------------

fn advice(p: &Project, id: &str) -> Vec<String> {
    p.json(&["prompt", id, "--json"])["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn each_check_fires_on_a_prompt_that_needs_it_and_not_on_one_that_does_not() {
    let p = Project::new();
    // Well formed: context, criteria, a dependency that said what it found.
    p.add("Upstream", &[]);
    p.expect(&["close", "1", "--result", "The answer."]);
    p.add(
        "Good",
        &[
            "-d",
            "1",
            "--body",
            "## Problem\n\nReal context.\n\n## Acceptance criteria\n\n- [ ] it holds\n",
        ],
    );
    assert_eq!(advice(&p, "2"), Vec::<String>::new());

    // No criteria, and nothing but headings.
    p.add("Bare", &["--body", "## Problem\n\n## Approach\n"]);
    let a = advice(&p, "3").join("\n");
    assert_contains(&a, "no acceptance criteria", "");
    assert_contains(&a, "no context", "");

    // A dependency that finished without saying what it concluded.
    p.add("Silent upstream", &[]);
    p.expect(&["close", "4"]);
    p.add(
        "Downstream",
        &["-d", "4", "--body", "## Problem\n\nContext.\n\n- [ ] one\n"],
    );
    assert_contains(
        &advice(&p, "5").join("\n"),
        "0004 finished without a result",
        "",
    );

    // Ticked, and nothing says how.
    p.add(
        "Ticked",
        &["--body", "## Problem\n\nContext.\n\n- [x] proven somehow\n"],
    );
    assert_contains(
        &advice(&p, "6").join("\n"),
        "1 criteria ticked, and no note",
        "",
    );
    p.expect(&["note", "6", "Proven by the test in tests/x.rs."]);
    assert_eq!(advice(&p, "6"), Vec::<String>::new(), "a note answers it");

    // Finished work and containers are not advised.
    p.expect(&["close", "3"]);
    assert_eq!(advice(&p, "3"), Vec::<String>::new());
}

#[test]
fn a_prompt_says_what_may_make_it_misread() {
    let p = Project::new();
    p.add("Bare", &["--body", "## Problem\n"]);
    let out = p.expect(&["prompt", "1"]).stdout;
    assert_contains(&out, "## This prompt may be misread", "");
    assert_contains(&out, "- no acceptance criteria", "");
}

#[test]
fn check_reports_prompts_only_when_asked() {
    let p = Project::new();
    p.add("Bare", &["--body", "## Problem\n"]);
    let plain = p.expect(&["check", "--strict"]);
    assert_missing(&plain.all(), "prompt:", "unchanged unless asked");
    let asked = p.expect(&["check", "--prompts"]);
    assert_contains(&asked.all(), "prompt: no acceptance criteria", "");
    assert_contains(
        &p.fails(&["check", "--prompts", "--strict"]).all(),
        "prompt:",
        "strict when asked",
    );
}

/// A bug filed from its template has scaffolding and nothing else.
#[test]
fn a_template_left_as_it_was_is_no_context() {
    let p = Project::with_init(&["init", "--name", "T"]);
    let bug = p.add("Filed from the template", &["-t", "bug"]);
    assert_contains(&advice(&p, &bug).join("\n"), "no context", "");
}

/// A handoff is a note, and proves nothing about a tick.
#[test]
fn a_handoff_does_not_explain_a_tick() {
    let p = Project::new();
    p.add(
        "Ticked",
        &["--body", "## Problem\n\nContext.\n\n- [x] done somehow\n"],
    );
    p.expect(&["claim", "1"]);
    p.expect(&["release", "1", "--reason", "ran out of time"]);
    assert_contains(&advice(&p, "1").join("\n"), "no note says how", "");
}

/// A dropped dependency did not finish, and closing it would say it had.
#[test]
fn a_dropped_dependency_is_named_for_what_it_is() {
    let p = Project::new();
    p.add("Abandoned", &[]);
    p.expect(&["close", "1", "--status", "dropped"]);
    p.add(
        "Rests on it",
        &["-d", "1", "--body", "## Problem\n\nContext.\n\n- [ ] one\n"],
    );
    let a = advice(&p, "2").join("\n");
    assert_contains(&a, "0001 was dropped", "");
    assert_missing(&a, "finished without a result", "");
}

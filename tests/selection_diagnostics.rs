// What `next` says when a selection comes back empty, and whether its counts
// describe the same work its list does.
mod support;
use support::*;

/// The shape that hid a whole milestone's work from agents: new work lands in
/// `backlog`, the selection view admits only `planned`.
fn narrow_view() -> (Project, [String; 3]) {
    let p = Project::with(Schema::standard().view("next", "status=planned"));
    let a = p.add("A", &[]);
    let b = p.add("B", &["-d", &a]);
    let c = p.add("C", &["-s", "planned", "-d", &b]);
    // Neither of these is blocked work: one is finished, the other a container.
    p.add("Finished early", &["-s", "done", "-d", &a]);
    let first = p.add("First release", &["-t", "milestone"]);
    p.add("Second release", &["-t", "milestone", "-d", &first]);
    (p, [a, b, c])
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

#[test]
fn blocked_is_counted_over_the_same_work_everywhere() {
    let (p, [_, b, c]) = narrow_view();

    let summary = p.expect(&["next"]).stdout;
    assert_contains(&summary, "1 ready · 2 blocked", "open work only");

    let hint = p.expect(&["next", "--view", "next"]).stderr;
    assert_contains(
        &hint,
        "1 item(s) in view `next` are blocked",
        "scoped to the view",
    );

    let listed = p
        .expect(&[
            "list",
            "--view",
            "next",
            "--filter",
            "blocked=true",
            "--count",
        ])
        .trimmed();
    let offered = p
        .expect(&["next", "--view", "next", "--blocked", "--count"])
        .trimmed();
    assert_eq!(listed, "1");
    assert_eq!(offered, listed, "next --blocked and list agree");
    assert_eq!(
        sorted(p.expect(&["next", "--blocked", "--ids"]).lines()),
        sorted(vec![b, c])
    );
}

#[test]
fn blocked_shows_blocked_work_and_what_it_waits_for() {
    let (p, [a, b, c]) = narrow_view();

    let out = p.expect(&["next", "--blocked"]).stdout;
    let row = out
        .lines()
        .find(|l| l.contains(&c))
        .unwrap_or_else(|| panic!("C is listed:\n{out}"));
    assert_contains(row, &b, "with the item in its way");
    let listed = p.expect(&["next", "--blocked", "--ids"]).stdout;
    assert_missing(&listed, &a, "ready work is not blocked work");

    // Nothing blocked in scope says that, not "nothing is ready".
    let none = p.expect(&["next", "--blocked", "--filter", "status=done"]);
    assert_contains(&none.stderr, "nothing is blocked", "the question asked");
    assert_missing(&none.stderr, "ready to start", "not the question asked");
}

#[test]
fn the_blocked_hint_repeats_the_selection_it_was_given() {
    let (p, _) = narrow_view();
    let hint = p.expect(&["next", "--view", "next"]).stderr;
    assert_contains(
        &hint,
        "`cairn next --view next --blocked`",
        "keeps the view",
    );

    let hint = p
        .expect(&["next", "--view", "next", "--filter", "type=feature|bug"])
        .stderr;
    assert_contains(
        &hint,
        "`cairn next --view next --filter 'type=feature|bug' --blocked`",
        "keeps the filter, quoted for a shell",
    );
    assert_contains(
        &hint,
        "outside it — `cairn next --filter 'type=feature|bug'` to see them",
        "drops only the view",
    );
}

#[test]
fn counting_the_selection_again_does_not_repeat_its_warnings() {
    let p = Project::with(Schema::standard().view("next", "status=planned,colour=red"));
    let a = p.add("A", &[]);
    p.add("B", &["-s", "planned", "-d", &a]);
    let err = p.expect(&["next", "--view", "next"]).stderr;
    assert_eq!(err.matches("names `colour`").count(), 1, "{err}");
}

#[test]
fn an_empty_view_says_how_much_is_ready_outside_it() {
    let (p, _) = narrow_view();
    let err = p.expect(&["next", "--view", "next"]).stderr;
    assert_contains(
        &err,
        "nothing is ready in view `next` (status=planned)",
        "names the view",
    );
    assert_contains(
        &err,
        "1 ready item(s) outside it — `cairn next` to see them",
        "and what it leaves out",
    );
    assert_missing(&err, "nothing is ready to start", "which was false");
}

#[test]
fn agent_and_check_note_a_view_that_leaves_agents_nothing() {
    let (p, [a, ..]) = narrow_view();

    let out = p.expect(&["agent", "--view", "next", "--write", "CLAUDE.md"]);
    assert_contains(
        &out.stderr,
        "new items start as `backlog`, which view `next` excludes",
        "said before anyone relies on it",
    );
    assert_contains(&out.stderr, "1 ready item(s) outside it", "and the cost");

    let check = p.expect(&["check", "--strict"]);
    assert_contains(&check.stderr, "note:", "said, not failed");
    assert_contains(
        &check.stderr,
        "CLAUDE.md selects view `next`",
        "names the source",
    );
    assert_contains(&check.stderr, "1 ready item(s) outside it", "and the cost");

    // Once the view holds ready work there is nothing to say.
    p.expect(&["set", &a, "status=planned"]);
    let check = p.expect(&["check", "--strict"]);
    assert_missing(&check.stderr, "selects view", "the queue has work");
}

#[test]
fn a_view_that_admits_new_work_draws_no_note() {
    let p = Project::with(Schema::standard().view("next", "status=backlog|planned"));
    p.add("A", &[]);
    let out = p.expect(&["agent", "--view", "next", "--write", "AGENTS.md"]);
    assert_missing(&out.stderr, "note:", "nothing is hidden");
}

#[test]
fn the_regenerate_line_names_the_file_it_was_written_to() {
    let (p, _) = narrow_view();
    p.expect(&["agent", "--view", "next", "--write", "CLAUDE.md"]);
    let written = std::fs::read_to_string(p.path("CLAUDE.md")).unwrap();
    assert_contains(
        &written,
        "`cairn agent --view next --write CLAUDE.md`",
        "the file it is in",
    );

    p.write("docs/AGENTS.md", "");
    let absolute = p.path("docs/AGENTS.md");
    p.expect(&[
        "agent",
        "--view",
        "next",
        "--write",
        absolute.to_str().unwrap(),
    ]);
    let written = std::fs::read_to_string(&absolute).unwrap();
    assert_contains(
        &written,
        "`cairn agent --view next --write docs/AGENTS.md`",
        "relative to the project, as a command run there needs it",
    );
}

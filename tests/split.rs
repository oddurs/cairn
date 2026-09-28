//! Splitting a prompt's numbered steps into sub-prompts.
mod support;
use serde_json::json;
use support::*;

const APPROACH: &str = "## Problem\n\nThree repositories, one change.\n\n## Approach\n\n\
1. In the harrow repository, file and do the reader change: numeric `id`.\n   \
Pin agreement with cairn.\n\
2. **Install** the matching pair.\n\
3. Migrate harrow, rim and nun, each in its own commit.\n\n\
## Acceptance criteria\n\n- [ ] All three are format 5\n";

fn deps(p: &Project, id: &str) -> serde_json::Value {
    p.json(&["show", id, "--json"])["depends_on"].clone()
}

#[test]
fn each_numbered_step_becomes_an_item_after_the_one_before() {
    let p = Project::new();
    p.milestone("v1.0", None);
    p.add(
        "Migrate three backlogs",
        &["-m", "v1.0", "--set", "priority=p0", "--body", APPROACH],
    );

    let out = p.expect(&["split", "2"]).stdout;
    assert_contains(
        &out,
        "created 0003  In the harrow repository, file and do the reader change",
        "",
    );
    assert_contains(
        &out,
        "created 0004  Install the matching pair",
        "emphasis removed",
    );
    assert_contains(&out, "0002 now depends on 0003, 0004, 0005", "");

    assert_eq!(deps(&p, "3"), json!([]));
    assert_eq!(deps(&p, "4"), json!([3]), "in order");
    assert_eq!(deps(&p, "5"), json!([4]));
    assert_eq!(deps(&p, "2"), json!([3, 4, 5]), "the parent finishes last");

    let child = p.json(&["show", "3", "--json"]);
    assert_eq!(child["milestone"], "v1.0", "filed where the parent is");
    assert_eq!(child["fields"]["priority"], "p0", "as urgent as the parent");
    let body = child["body"].as_str().unwrap();
    assert_contains(
        body,
        "Step 1 of 0002, Migrate three backlogs.",
        "where it came from",
    );
    assert_contains(body, "Pin agreement with cairn.", "the whole step");

    let parent = p.json(&["show", "2", "--json"])["body"].clone();
    assert_contains(
        parent.as_str().unwrap(),
        "Split into 0003, 0004, 0005",
        "a note says so",
    );
    p.expect(&["check"]);
}

#[test]
fn parallel_steps_have_no_order_between_them() {
    let p = Project::new();
    p.add("Parallel", &["--body", APPROACH]);
    p.expect(&["split", "1", "--parallel"]);
    for id in ["2", "3", "4"] {
        assert_eq!(deps(&p, id), json!([]), "{id}");
    }
    assert_eq!(deps(&p, "1"), json!([2, 3, 4]));
}

#[test]
fn a_part_of_field_names_the_parent() {
    // The standard preset declares `part_of`, a rollup reference by id.
    let p = Project::with_init(&["init", "--name", "T"]);
    let parent = p.add("A composed piece of work", &["--body", APPROACH]);
    p.expect(&["split", &parent]);
    let doc = p.json(&["list", "-A", "--json"]);
    let children: Vec<&serde_json::Value> = doc
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| {
            i["title"].as_str().is_some_and(|t| {
                t.starts_with("In the harrow")
                    || t.starts_with("Install")
                    || t.starts_with("Migrate harrow")
            })
        })
        .collect();
    assert_eq!(children.len(), 3);
    for c in children {
        let n: u64 = parent.trim_start_matches('0').parse().unwrap();
        assert_eq!(c["fields"]["part_of"], json!([n]), "{c}");
    }
    p.expect(&["check"]);
}

#[test]
fn a_dry_run_writes_nothing_and_says_what_it_would_create() {
    let p = Project::new();
    p.add("Plan", &["--body", APPROACH]);
    let before = p.files("cairn/items");
    let out = p.expect(&["split", "1", "--dry-run"]).stdout;
    assert_contains(&out, "3 item(s) would be created", "");
    assert_contains(&out, "after 0002", "the order");
    assert_eq!(p.files("cairn/items"), before);
}

#[test]
fn an_item_without_steps_or_already_split_is_refused_with_the_reason() {
    let p = Project::new();
    p.add("No steps", &["--body", "## Approach\n\nJust do it.\n"]);
    assert_contains(&p.fails(&["split", "1"]).all(), "0 numbered step(s)", "");
    p.add("No approach", &["--body", "## Problem\n\nHm.\n"]);
    assert_contains(&p.fails(&["split", "2"]).all(), "no `Approach` section", "");

    p.add("Split once", &["--body", APPROACH]);
    p.expect(&["split", "3"]);
    assert_contains(&p.fails(&["split", "3"]).all(), "split already", "");

    p.add("Elsewhere", &["--body", "## Steps\n\n1. One\n2. Two\n"]);
    p.expect(&["split", "7", "--from", "Steps"]);
}

/// What `new` would do: schema defaults, and a required field passed on from
/// the parent — or a refusal that writes nothing when there is none to pass.
#[test]
fn children_are_filed_as_new_would_file_them() {
    let p = Project::with(Schema::standard().amend("area", Field::required));
    p.add("Needs an area", &["--set", "area=cli", "--body", APPROACH]);
    p.expect(&["split", "1"]);
    assert_eq!(p.json(&["show", "2", "--json"])["fields"]["area"], "cli");
    p.expect(&["check"]);

    let q = Project::with(Schema::standard().amend("area", Field::required));
    q.write_item(
        "0001-no-area.md",
        "id: 1\ntitle: No area\ntype: feature\nstatus: backlog",
        APPROACH,
    );
    let before = q.files("cairn/items");
    assert_contains(
        &q.fails(&["split", "1"]).all(),
        "field `area` is required",
        "",
    );
    assert_eq!(q.files("cairn/items"), before, "nothing was written");
}

/// A milestone's steps are work, not milestones with no key.
#[test]
fn a_containers_steps_take_the_default_type() {
    let p = Project::new();
    p.add(
        "A release",
        &["-t", "milestone", "--set", "key=r1", "--body", APPROACH],
    );
    p.expect(&["split", "1"]);
    assert_eq!(p.json(&["show", "2", "--json"])["type"], "feature");
    p.expect(&["check"]);
}

/// Only a note records a split; a paragraph that begins "Split into" does not.
#[test]
fn a_paragraph_is_not_a_record_of_a_split() {
    let p = Project::new();
    p.add(
        "Crates",
        &[
            "--body",
            &format!("Split into two crates so that each builds alone.\n\n{APPROACH}"),
        ],
    );
    p.expect(&["split", "1"]);
}

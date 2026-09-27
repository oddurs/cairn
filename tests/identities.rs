//! Format 5's identity contract, exercised through the real executable: a
//! readable number people see and type, and a `uid` tag that outlives it.
mod support;
use serde_json::{Value, json};
use support::*;

const A: &str = "a83f26b1-0000-4000-8000-000000000001";
const B: &str = "a83f26b1-1000-4000-8000-000000000002";
const C: &str = "b93f26b1-0000-4000-8000-000000000003";

fn id(p: &Project, reference: &str) -> String {
    id_text(&p.json(&["show", reference, "--json"])["id"])
}

fn titled(p: &Project, title: &str) -> String {
    p.json(&["list", "-A", "--json"])
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["title"] == title)
        .map_or_else(|| panic!("no item titled {title}"), |i| id_text(&i["id"]))
}

fn uid(p: &Project, reference: &str) -> String {
    p.json(&["show", reference, "--json"])["uid"]
        .as_str()
        .unwrap_or_default()
        .into()
}

fn item_path(p: &Project, reference: &str) -> String {
    p.json(&["show", reference, "--json"])["path"]
        .as_str()
        .unwrap()
        .into()
}

fn references(p: &Project) {
    p.append("cairn.toml", "\n[[field]]\nname = \"related\"\nkind = \"ref\"\nby = \"id\"\ncardinality = \"many\"\ntarget = \"*\"\n");
}

fn with_format(p: &Project, n: u32) {
    p.write(
        "cairn.toml",
        &p.read("cairn.toml")
            .replace("format = 5", &format!("format = {n}")),
    );
}

/// A format-3 project with a project template, a CRLF item carrying metadata
/// nobody declared, and references by id.
fn format_three() -> Project {
    let p = Project::new();
    with_format(&p, 3);
    p.write(
        "cairn.toml",
        &p.read("cairn.toml")
            .replace("[project]", "[project]\nid_format = \"CRN-{n:04}\""),
    );
    references(&p);
    p.write("cairn/items/CRN-0067-first.md", "---\r\nid: 67\r\ntitle: First\r\ntype: feature\r\nstatus: backlog\r\ncreated: 2020-02-03\r\nupdated: 2021-04-05\r\nopaque: {kept: true}\r\n---\r\n\r\nProse names #68, unchanged.  \r\n\r\n");
    p.write_item(
        "CRN-0068-second.md",
        "id: 68\ntitle: Second\ntype: feature\nstatus: backlog\ndepends_on: [67]\nrelated: [67]",
        "Another body.\n",
    );
    p
}

/// The same project after a format-4 migration, plus one item created since,
/// which never had a number.
fn format_four() -> Project {
    let p = Project::new();
    with_format(&p, 4);
    references(&p);
    p.write(
        "cairn/items/_legacy-ids.toml",
        &format!(
            "version = 1\nid_format = \"CRN-{{n:04}}\"\n\n[ids]\n67 = \"{A}\"\n68 = \"{B}\"\n"
        ),
    );
    p.write("cairn/items/CRN-0067-first.md", &format!("---\r\nid: {A}\r\ntitle: First\r\ntype: feature\r\nstatus: backlog\r\ncreated: 2020-02-03\r\nupdated: 2021-04-05\r\nopaque: {{kept: true}}\r\n---\r\n\r\nProse names #68, unchanged.  \r\n\r\n"));
    p.write_item(
        "CRN-0068-second.md",
        &format!("id: {B}\ntitle: Second\ntype: feature\nstatus: backlog\ndepends_on: [{A}]\nrelated: [{A}]"),
        "Another body.\n",
    );
    p.write_item(
        &format!("{C}-third.md"),
        &format!("id: {C}\ntitle: Third\ntype: feature\nstatus: backlog\ncreated: 2026-09-24\ndepends_on: [{B}]"),
        "Filed after format 4.\n",
    );
    p
}

// --- numbers and tags ---------------------------------------------------------

#[test]
fn fresh_items_are_numbered_and_tagged() {
    let p = Project::new();
    let reference = p.add("First", &[]);
    assert_eq!(reference, "0001");
    assert_eq!(id(&p, &reference), "1");
    let tag = uuid::Uuid::parse_str(&uid(&p, "1")).expect("a tag");
    assert_eq!(tag.get_version(), Some(uuid::Version::Random));
    assert!(item_path(&p, "1").ends_with("0001-first.md"));
    let file = p.read(&item_path(&p, "1"));
    assert!(
        file.starts_with(&format!("---\nid: 1\nuid: {tag}\n")),
        "the tag sits under the number: {file}"
    );
    // The tag is never how an item is shown.
    assert_missing(&p.expect(&["list"]).stdout, &tag.to_string()[..8], "tables");
    assert!(!p.exists("cairn/items/_legacy-ids.toml"));
    p.expect(&["check", "--strict"]);
}

#[test]
fn a_tag_resolves_whole_or_by_a_prefix_that_cannot_be_a_number() {
    let p = Project::new();
    p.write_item(
        "0001-first.md",
        &format!("id: 1\nuid: {A}\ntitle: First\nstatus: backlog"),
        "",
    );
    p.write_item(
        "0002-second.md",
        &format!("id: 2\nuid: {B}\ntitle: Second\nstatus: backlog"),
        "",
    );
    p.write_item(
        "0003-third.md",
        &format!("id: 3\nuid: {C}\ntitle: Third\nstatus: backlog"),
        "",
    );
    assert_eq!(id(&p, A), "1");
    assert_eq!(id(&p, &A.replace('-', "")), "1");
    assert_eq!(id(&p, "B93F26B1"), "3", "case does not matter");
    assert_contains(
        &p.fails(&["show", "a83f26b1"]).all(),
        "2 items",
        "an ambiguous prefix chooses nothing",
    );
    assert_eq!(id(&p, "a83f26b10"), "1");
    // All digits is always a number, never the start of a tag.
    assert_eq!(id(&p, "0002"), "2");
    // And a key that reads like a tag would make a reference ambiguous.
    p.fails(&["set", "1", "key=deadbeef"]);
    p.fails(&["set", "1", &format!("key={C}")]);
}

#[test]
fn a_type_writes_its_own_ids_from_the_one_counter() {
    let p = Project::new();
    p.write(
        "cairn.toml",
        &p.read("cairn.toml").replacen(
            "name = \"bug\"\n",
            "name = \"bug\"\nid_format = \"BUG-{n}\"\n",
            1,
        ),
    );
    let feature = p.add("A feature", &["-t", "feature"]);
    let bug = p.add("A bug", &["-t", "bug"]);
    assert_eq!(feature, "0001");
    assert_eq!(bug, "BUG-2", "one counter, the bug's own rendering");
    assert!(item_path(&p, "2").ends_with("BUG-2-a-bug.md"));
    for spelling in ["BUG-2", "bug-2", "2", "#2", "0002"] {
        assert_eq!(id(&p, spelling), "2", "{spelling}");
    }
    assert_contains(
        &p.fails(&["show", "BUG-1"]).all(),
        "1 is a feature",
        "a prefix naming the wrong type is refused, not reinterpreted",
    );

    // Retyped, it keeps its number and its tag; only how it reads changes.
    let tag = uid(&p, "2");
    p.expect(&["set", "2", "type=feature"]);
    assert_eq!(p.json(&["show", "2", "--json"])["ref"], "0002");
    assert_eq!(uid(&p, "2"), tag);
    assert!(item_path(&p, "2").ends_with("0002-a-bug.md"));
    p.expect(&["check", "--strict"]);
}

#[test]
fn a_type_template_must_say_which_type_it_is() {
    for (template, complaint) in [
        ("{n:03}", "no prefix or suffix"),
        ("{n:04}", "no prefix or suffix"),
    ] {
        let p = Project::new();
        p.write(
            "cairn.toml",
            &p.read("cairn.toml").replacen(
                "name = \"bug\"\n",
                &format!("name = \"bug\"\nid_format = \"{template}\"\n"),
                1,
            ),
        );
        assert_contains(&p.fails(&["list"]).all(), complaint, template);
    }
    let p = Project::new();
    p.write(
        "cairn.toml",
        &p.read("cairn.toml")
            .replacen(
                "name = \"bug\"\n",
                "name = \"bug\"\nid_format = \"X-{n}\"\n",
                1,
            )
            .replacen(
                "name = \"chore\"\n",
                "name = \"chore\"\nid_format = \"x-{n}\"\n",
                1,
            ),
    );
    assert_contains(
        &p.fails(&["list"]).all(),
        "reads the same",
        "two types, one spelling",
    );
}

// --- allocation -----------------------------------------------------------------

#[test]
fn a_number_held_by_a_sibling_worktree_or_any_branch_is_never_handed_out() {
    let p = repository();
    // Uncommitted, in another worktree: the parallel-agent case.
    let sibling = Project::empty();
    git(
        &p,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "agent",
            sibling.root().to_str().unwrap(),
        ],
    );
    let theirs = sibling.add("An agent's item", &[]);
    let ours = p.add("Mine", &[]);
    assert_ne!(theirs, ours);

    // Committed on a branch and then deleted: never reused.
    git(&p, &["checkout", "-qb", "short-lived"]);
    let gone = p.add("Short-lived", &[]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "short-lived"]);
    p.expect(&["remove", &gone, "--force"]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "gone"]);
    let next = p.add("After", &[]);
    for taken in [&theirs, &ours, &gone] {
        assert_ne!(&next, taken);
    }
}

// --- format 3 to 5 -------------------------------------------------------------

#[test]
fn migrating_format_three_adds_one_tag_line_and_nothing_else() {
    let p = format_three();
    let before = [
        p.read("cairn/items/CRN-0067-first.md"),
        p.read("cairn/items/CRN-0068-second.md"),
    ];
    p.expect(&["migrate"]);
    let after = [
        p.read("cairn/items/CRN-0067-first.md"),
        p.read("cairn/items/CRN-0068-second.md"),
    ];
    for (before, after) in before.iter().zip(&after) {
        let eol = if before.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let added: Vec<&str> = after
            .split(eol)
            .filter(|l| !before.split(eol).any(|b| b == *l))
            .collect();
        assert_eq!(added.len(), 1, "one line: {added:?}");
        assert!(added[0].starts_with("uid: "), "{added:?}");
        assert_eq!(after.replace(&format!("{}{eol}", added[0]), ""), *before);
    }
    assert_eq!(
        p.json(&["show", "CRN-0068", "--json"])["depends_on"],
        json!([67])
    );
    assert_ne!(uid(&p, "67"), uid(&p, "68"));
    p.expect(&["check"]);
}

// --- format 4 to 5 -------------------------------------------------------------

#[test]
fn format_four_is_read_but_not_written() {
    let p = format_four();
    assert_eq!(p.count_all(), 3);
    p.expect(&["show", A]);
    assert_contains(
        &p.fails(&["new", "Not yet"]).all(),
        "cairn migrate",
        "writing names the way forward",
    );
}

#[test]
fn migrating_format_four_restores_numbers_and_keeps_each_uuid_as_the_tag() {
    let p = format_four();
    let body = p.read("cairn/items/CRN-0067-first.md");
    let preview = p.expect(&["migrate", "--dry-run"]).stdout;
    assert_contains(&preview, "2 item(s) get back the number they had", "");
    assert_contains(
        &preview,
        "b93f26b1 -> CRN-0069  Third",
        "the new numbers, before",
    );
    assert_eq!(p.read("cairn/items/CRN-0067-first.md"), body, "a dry run");

    p.expect(&["migrate"]);
    p.expect(&["migrate", "--check"]);

    // The numbers items had before format 4, and the next one for the item
    // created since.
    assert_eq!(id(&p, "CRN-0067"), "67");
    assert_eq!(id(&p, "68"), "68");
    assert_eq!(
        titled(&p, "Third"),
        "69",
        "numbered after every restored number"
    );
    assert_eq!(uid(&p, "67"), A);
    assert_eq!(uid(&p, "69"), C);
    // An old reference in a commit trailer still finds its item.
    assert_eq!(id(&p, "b93f26b1"), "69");

    let second = p.json(&["show", "68", "--json"]);
    assert_eq!(second["depends_on"], json!([67]));
    assert_eq!(second["fields"]["related"], json!([67]));
    assert_eq!(p.json(&["show", "69", "--json"])["depends_on"], json!([68]));

    // The project's rendering comes back, legacy names stay, the UUID name goes.
    assert_contains(&p.read("cairn.toml"), "id_format = \"CRN-{n:04}\"", "");
    assert!(p.exists("cairn/items/CRN-0067-first.md"));
    assert!(p.exists("cairn/items/CRN-0069-third.md"));
    assert!(!p.exists(&format!("cairn/items/{C}-third.md")));
    assert!(!p.exists("cairn/items/_legacy-ids.toml"));

    // Bodies, dates, line endings and metadata nobody declared are untouched.
    let after = p.read("cairn/items/CRN-0067-first.md");
    assert_eq!(
        body.split_once("\r\n---\r\n").unwrap().1,
        after.split_once("\r\n---\r\n").unwrap().1
    );
    assert!(after.contains("opaque:"));
    assert!(!after.replace("\r\n", "").contains('\n'));
    assert_eq!(p.json(&["show", "67", "--json"])["created"], "2020-02-03");

    p.expect(&["check"]);
    assert_eq!(p.add("Next", &[]), "CRN-0070");
}

#[test]
fn migrating_format_four_refuses_a_backlog_that_does_not_check() {
    let p = format_four();
    p.write_item(
        "dangling.md",
        &format!(
            "id: {}\ntitle: Dangling\nstatus: backlog\ndepends_on: [{}]",
            "c93f26b1-0000-4000-8000-000000000009", "d93f26b1-0000-4000-8000-00000000000d"
        ),
        "",
    );
    let before = p.read("cairn/items/CRN-0067-first.md");
    p.fails(&["migrate"]);
    assert_eq!(p.read("cairn/items/CRN-0067-first.md"), before);
    assert!(p.exists("cairn/items/_legacy-ids.toml"));
    assert_contains(&p.read("cairn.toml"), "format = 4", "nothing moved");
}

/// Reconstruct a genuine interrupted migration from a completed one: every
/// file's before and after, a journal recording them, and the first write
/// already made. No fault injection in the program itself.
fn interrupted() -> (Project, Value) {
    let p = format_four();
    let snapshot = |p: &Project| -> Vec<(String, String)> {
        let mut files: Vec<(String, String)> = p
            .files("cairn/items")
            .into_iter()
            .filter(|f| {
                f == "_legacy-ids.toml"
                    || std::path::Path::new(f)
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("md"))
            })
            .map(|f| format!("cairn/items/{f}"))
            .map(|f| (f.clone(), p.read(&f)))
            .collect();
        files.sort();
        files
    };
    let before = snapshot(&p);
    let config = p.read("cairn.toml");
    p.expect(&["migrate"]);
    let after = snapshot(&p);

    let mut writes = Vec::new();
    let mut removals = Vec::new();
    for (path, text) in &after {
        let was = before
            .iter()
            .find(|(b, _)| b == path)
            .map(|(_, t)| t.clone());
        writes.push(json!({"path": path, "before": was, "after": text}));
    }
    for (path, text) in &before {
        if !after.iter().any(|(a, _)| a == path) {
            removals.push(json!({"path": path, "before": text, "after": null}));
        }
    }
    let mut files = writes;
    files.extend(removals);
    files.push(json!({"path": "cairn.toml", "before": config, "after": p.read("cairn.toml")}));

    for (path, _) in &after {
        p.remove(path);
    }
    for (path, text) in &before {
        p.write(path, text);
    }
    p.write("cairn.toml", &config);
    let plan = json!({"version": 2, "target": 5, "files": files});
    p.write("cairn/items/.identity-migration.json", &plan.to_string());
    let first = &plan["files"][0];
    p.write(
        first["path"].as_str().unwrap(),
        first["after"].as_str().unwrap(),
    );
    (p, plan)
}

#[test]
fn an_interrupted_migration_blocks_everything_and_resumes_exactly() {
    let (p, plan) = interrupted();
    p.fails(&["show", A]);
    p.fails(&["new", "Must not appear"]);
    p.fails(&["migrate", "--check"]);
    p.expect(&["migrate", "--dry-run"]);
    assert!(p.exists("cairn/items/.identity-migration.json"));

    p.expect(&["migrate"]);
    for file in plan["files"].as_array().unwrap() {
        let path = file["path"].as_str().unwrap();
        match file["after"].as_str() {
            Some(text) => assert_eq!(p.read(path), text, "{path}"),
            None => assert!(!p.exists(path), "{path} should be gone"),
        }
    }
    assert!(!p.exists("cairn/items/.identity-migration.json"));
    assert_eq!(titled(&p, "Third"), "69");
    p.expect(&["check"]);
}

#[test]
fn a_resumed_migration_refuses_an_intervening_edit_or_an_unsafe_path() {
    let (p, _) = interrupted();
    p.append("cairn/items/CRN-0068-second.md", "A concurrent edit.\n");
    let before = p.read("cairn/items/CRN-0068-second.md");
    assert_contains(
        &p.fails(&["migrate"]).all(),
        "changed since",
        "never overwrite intervening edits",
    );
    assert_eq!(p.read("cairn/items/CRN-0068-second.md"), before);

    let (p, mut plan) = interrupted();
    plan["files"][0]["path"] = json!("../outside.md");
    p.write("cairn/items/.identity-migration.json", &plan.to_string());
    assert_contains(&p.fails(&["migrate"]).all(), "unsafe", "reject traversal");
}

/// One piece of work, followed through all three identity eras: a number in
/// format 3, a UUID in format 4, a number and a tag in format 5.
#[test]
fn history_follows_an_item_across_every_format() {
    let p = format_three();
    git(&p, &["init", "-q", "-b", "main"]);
    git(&p, &["add", "."]);
    git(&p, &["commit", "-qm", "format 3"]);
    let start = git(&p, &["rev-parse", "HEAD"]).trim().to_string();

    // What the format-4 migration wrote, by hand, since no cairn writes it now.
    let four = format_four();
    for f in [
        "cairn.toml",
        "cairn/items/_legacy-ids.toml",
        "cairn/items/CRN-0067-first.md",
        "cairn/items/CRN-0068-second.md",
    ] {
        p.write(f, &four.read(f));
    }
    git(&p, &["add", "."]);
    git(&p, &["commit", "-qm", "format 4"]);

    p.expect(&["migrate"]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "format 5"]);

    let range = p.json(&["log", "--range", &format!("{start}..HEAD"), "--json"]);
    assert!(
        range["items"].as_array().unwrap().is_empty(),
        "changing how identity is written is not work: {range}"
    );
    p.expect(&["set", "67", "title=Renamed"]);
    git(&p, &["add", "-A"]);
    git(&p, &["commit", "-qm", "retitle"]);
    let history = p.json(&["log", "67", "--json"]);
    assert_eq!(
        history["revisions"].as_array().unwrap().len(),
        4,
        "{history}"
    );
    assert_eq!(
        p.json(&["log", A, "--json"]),
        history,
        "by its old UUID too"
    );
}

// --- work from format-4 branches, merged later --------------------------------

#[test]
fn an_item_arriving_with_a_format_four_identity_is_numbered_by_renumber() {
    let p = Project::new();
    p.add("Already here", &[]);
    // Filed on a branch while the project was format 4, merged after.
    p.write_item(
        &format!("{C}-late.md"),
        &format!("id: {C}\ntitle: Late\nstatus: backlog"),
        "",
    );
    p.write_item(
        "0002-refers.md",
        &format!("id: 2\ntitle: Refers\nstatus: backlog\ndepends_on: [{C}]"),
        "",
    );
    assert_contains(&p.fails(&["check"]).all(), "cairn renumber", "the remedy");
    p.expect(&["renumber"]);
    assert_eq!(titled(&p, "Late"), "3");
    assert_eq!(uid(&p, "3"), C, "its UUID became its tag");
    assert_eq!(p.json(&["show", "2", "--json"])["depends_on"], json!([3]));
    assert!(item_path(&p, "3").ends_with("0003-late.md"));
    p.expect(&["check", "--strict"]);
}

#[test]
fn a_copied_file_is_reported_by_its_tag() {
    let p = Project::new();
    let first = p.add("Original", &[]);
    let copy = p.read(&item_path(&p, &first)).replace("id: 1\n", "id: 2\n");
    p.write("cairn/items/0002-copy.md", &copy);
    assert_contains(&p.fails(&["check"]).all(), "a copied file", "");
}

// --- interchange ----------------------------------------------------------------

#[test]
fn a_format_four_export_imports_by_tag_and_updates_in_place() {
    let p = Project::new();
    references(&p);
    p.write("import.json", &json!({"cairn":"2", "items":[
        {"id": A, "title":"First", "status":"done", "depends_on":[B], "fields":{"related":[B]}, "closed_at":"2026-01-02"},
        {"id": B, "title":"Second"}
    ]}).to_string());
    p.expect(&["import", "import.json"]);
    let first = id(&p, A);
    let second = id(&p, B);
    assert_eq!(first, "1");
    let item = p.json(&["show", &first, "--json"]);
    assert_eq!(item["depends_on"], json!([2]));
    assert_eq!(item["fields"]["related"], json!([2]));
    assert_eq!(item["closed_at"], "2026-01-02");

    // Numbers are local; tags are not. A second import recognises its work.
    p.expect(&["import", "import.json", "--update"]);
    assert_eq!(p.count_all(), 2);
    assert_eq!(id(&p, B), second);

    let export = p.json(&["export"]);
    assert_eq!(export["cairn"], "1");
    let receiver = Project::new();
    references(&receiver);
    receiver.add("Something of its own", &[]);
    receiver.write("import.json", &export.to_string());
    receiver.expect(&["import", "import.json"]);
    assert_eq!(uid(&receiver, &id(&receiver, A)), A);
    assert_eq!(
        receiver.json(&["show", A, "--json"])["depends_on"],
        json!([id(&receiver, B).parse::<u64>().unwrap()])
    );
    receiver.expect(&["check", "--strict"]);
}

#[test]
fn duplicate_import_identities_are_rejected_before_writing_any_items() {
    let p = Project::new();
    for id in [json!(A), json!(67)] {
        p.write(
            "import.json",
            &json!([{"id":id,"title":"First"},{"id":id,"title":"Second"}]).to_string(),
        );
        p.fails(&["import", "import.json"]);
        assert_eq!(p.count_all(), 0);
    }
}

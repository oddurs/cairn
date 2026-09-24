//! Format 4's identity contract, exercised through the real executable.
mod support;
use serde_json::{Value, json};
use support::*;

const A: &str = "a83f26b1-0000-4000-8000-000000000001";
const B: &str = "a83f26b1-1000-4000-8000-000000000002";
const C: &str = "b93f26b1-0000-4000-8000-000000000003";

fn full(p: &Project, reference: &str) -> String {
    p.json(&["show", reference, "--json"])["id"]
        .as_str()
        .unwrap()
        .into()
}

fn item_path(p: &Project, reference: &str) -> String {
    p.json(&["show", reference, "--json"])["path"]
        .as_str()
        .unwrap()
        .into()
}

fn uuid_item(p: &Project, id: &str, title: &str, extra: &str) {
    p.write_item(
        &format!("{id}-{}.md", title.to_lowercase()),
        &format!("id: {id}\ntitle: {title}\ntype: feature\nstatus: backlog\n{extra}"),
        "Body.\n",
    );
}

fn references(p: &Project) {
    p.append("cairn.toml", "\n[[field]]\nname = \"related\"\nkind = \"ref\"\nby = \"id\"\ncardinality = \"many\"\ntarget = \"*\"\n");
}

fn legacy() -> Project {
    let p = Project::new();
    p.write(
        "cairn.toml",
        &p.read("cairn.toml")
            .replace("format = 4", "format = 3")
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

#[test]
fn fresh_projects_use_full_uuid_v4_with_short_command_references() {
    let p = Project::new();
    let reference = p.add("First", &[]);
    assert_eq!(reference.len(), 8);
    let id = full(&p, &reference);
    let parsed = uuid::Uuid::parse_str(&id).unwrap();
    assert_eq!(parsed.get_version(), Some(uuid::Version::Random));
    assert_eq!(parsed.get_variant(), uuid::Variant::RFC4122);
    assert_eq!(p.json(&["show", &id, "--json"])["ref"], reference);
    assert!(item_path(&p, &id).contains(&id));
    assert!(p.item_file(&id).contains(&format!("id: {id}")));
    assert!(!p.exists("cairn/items/_legacy-ids.toml"));
    p.fails(&["show", "1"]);
    p.fails(&["set", &id, &format!("id={C}")]);
    p.expect(&["check", "--strict"]);
}

#[test]
fn prefixes_are_case_insensitive_and_never_choose_an_ambiguous_item() {
    let p = Project::new();
    uuid_item(&p, A, "First", "");
    uuid_item(&p, B, "Second", "");
    let error = p.fails(&["show", "a83f26b1"]).all();
    assert_contains(&error, "ambiguous", "do not choose a candidate");
    assert_contains(&error, A, "list complete candidates");
    assert_contains(&error, B, "list complete candidates");
    p.fails(&["list", "--filter", "id=a83f26b1"]);
    assert_eq!(full(&p, "A83F26B10"), A);
    assert_eq!(full(&p, &A.replace('-', "")), A);
    p.fails(&["show", "a83f26b"]);
    let rows = p.json(&["list", "--json"]);
    assert!(
        rows.as_array()
            .unwrap()
            .iter()
            .all(|row| row["ref"].as_str().unwrap().len() == 9)
    );
    p.fails(&["set", A, "key=deadbeef"]);
    p.fails(&["set", A, &format!("key={C}")]);
}

#[test]
fn full_identities_are_stored_for_all_reference_fields() {
    let p = Project::new();
    references(&p);
    let first = p.add("First", &[]);
    let second = p.add("Second", &[]);
    let id = full(&p, &first);
    p.expect(&[
        "set",
        &second,
        &format!("depends_on={first}"),
        &format!("related={first}"),
    ]);
    let item = p.json(&["show", &second, "--json"]);
    assert_eq!(item["depends_on"], json!([id]));
    assert_eq!(item["fields"]["related"], json!([id]));
    assert_eq!(p.count_of(&format!("related={first}")), 1);
    p.expect(&["set", &second, &format!("related-={first}")]);
    assert!(
        p.json(&["show", &second, "--json"])["fields"]
            .get("related")
            .is_none()
    );
    p.expect(&["check", "--strict"]);
}

#[test]
fn persisted_prefixes_and_numeric_identities_are_rejected() {
    for front in [
        "id: 67",
        "id: a83f26b1",
        "id: 00000000-0000-7000-8000-000000000000",
        "title: Missing identity",
    ] {
        let p = Project::new();
        p.write_item("0067-bad.md", front, "");
        p.fails(&["check"]);
    }
    for extra in [
        "depends_on: [a83f26b1]",
        "related: [a83f26b1]",
        "related: [67]",
    ] {
        let p = Project::new();
        references(&p);
        uuid_item(&p, C, "Bad", extra);
        p.fails(&["check"]);
    }
}

#[test]
fn migration_preserves_prose_dates_unknown_metadata_and_filenames() {
    let p = legacy();
    let old = p.read("cairn/items/CRN-0067-first.md");
    p.expect(&["migrate", "--dry-run"]);
    assert_eq!(p.read("cairn/items/CRN-0067-first.md"), old);
    assert!(!p.exists("cairn/items/_legacy-ids.toml"));
    p.expect(&["migrate"]);
    p.expect(&["migrate", "--check"]);
    let id = full(&p, "67");
    assert_eq!(full(&p, "0067"), id);
    assert_eq!(full(&p, "CRN-0067"), id);
    let first = p.json(&["show", "67", "--json"]);
    assert_eq!(first["created"], "2020-02-03");
    assert_eq!(first["updated"], "2021-04-05");
    let after = p.read("cairn/items/CRN-0067-first.md");
    assert_eq!(
        old.split_once("\r\n---\r\n").unwrap().1,
        after.split_once("\r\n---\r\n").unwrap().1
    );
    assert!(after.contains("opaque:"));
    assert!(!after.replace("\r\n", "").contains('\n'));
    let second = p.json(&["show", "68", "--json"]);
    assert_eq!(second["depends_on"], json!([id]));
    assert_eq!(second["fields"]["related"], json!([id]));
    assert_eq!(p.count_of("id=67"), 1);
    p.expect(&["renumber", "-q"]);
    assert!(p.exists("cairn/items/CRN-0067-first.md"));
    let map = p.read("cairn/items/_legacy-ids.toml");
    p.add("New identity", &[]);
    assert_eq!(
        p.read("cairn/items/_legacy-ids.toml"),
        map,
        "no counter or extending alias registry"
    );
    p.expect(&["check"]);
}

#[test]
fn ordinary_edits_preserve_migrated_filenames_until_the_title_changes() {
    let p = legacy();
    // Unknown-metadata preservation is covered separately; this case should
    // also pass strict validation before and after the filename decisions.
    let path = "cairn/items/CRN-0067-first.md";
    p.write(path, &p.read(path).replace("opaque: {kept: true}\r\n", ""));
    p.expect(&["migrate"]);
    let id = full(&p, "67");
    let before = item_path(&p, "67");
    p.expect(&["set", "67", "status=doing", "priority=p2"]);
    assert_eq!(item_path(&p, "67"), before);
    p.expect(&["check", "--strict"]);
    p.expect(&["set", "67", "title=Renamed"]);
    assert_eq!(full(&p, "67"), id);
    assert_ne!(item_path(&p, "67"), before);
    assert!(item_path(&p, "67").ends_with(&format!("{id}-renamed.md")));
    p.expect(&["check", "--strict"]);
}

#[test]
fn migration_refuses_conflicting_or_dangling_legacy_identities_without_changes() {
    for front in [
        "id: 67\ntitle: Duplicate\nstatus: backlog",
        "id: 69\ntitle: Dangling\nstatus: backlog\ndepends_on: [70]",
    ] {
        let p = legacy();
        p.write_item("CRN-0069-bad.md", front, "");
        let before = p.read("cairn/items/CRN-0067-first.md");
        p.fails(&["migrate"]);
        assert_eq!(p.read("cairn/items/CRN-0067-first.md"), before);
        assert!(!p.exists("cairn/items/_legacy-ids.toml"));
        assert!(p.read("cairn.toml").contains("format = 3"));
    }
}

/// Reconstruct a genuine interrupted migration from a successfully generated
/// plan. No production fault-injection switch, fabricated identities, or
/// normalization of command output is involved.
fn interrupted() -> (Project, Value, String) {
    let p = legacy();
    let paths = [
        "cairn/items/CRN-0067-first.md",
        "cairn/items/CRN-0068-second.md",
        "cairn/items/_legacy-ids.toml",
        "cairn.toml",
    ];
    let before: Vec<Option<String>> = paths
        .iter()
        .map(|path| p.exists(path).then(|| p.read(path)))
        .collect();
    p.expect(&["migrate"]);
    let id = full(&p, "67");
    let files: Vec<Value> = paths
        .iter()
        .zip(before)
        .map(|(path, before)| json!({"path": path, "before": before, "after": p.read(path)}))
        .collect();
    for file in &files {
        let path = file["path"].as_str().unwrap();
        match file["before"].as_str() {
            Some(text) => p.write(path, text),
            None => p.remove(path),
        }
    }
    let plan = json!({"version": 1, "files": files});
    p.write("cairn/items/.identity-migration.json", &plan.to_string());
    p.write(paths[0], plan["files"][0]["after"].as_str().unwrap());
    (p, plan, id)
}

#[test]
fn interrupted_migration_blocks_other_commands_and_resumes_the_same_identities() {
    let (p, plan, id) = interrupted();
    p.fails(&["show", "67"]);
    p.fails(&["new", "Must not appear"]);
    p.fails(&["migrate", "--check"]);
    p.expect(&["migrate", "--dry-run"]);
    assert!(p.exists("cairn/items/.identity-migration.json"));
    p.expect(&["migrate"]);
    assert_eq!(full(&p, "67"), id);
    for file in plan["files"].as_array().unwrap() {
        assert_eq!(
            p.read(file["path"].as_str().unwrap()),
            file["after"].as_str().unwrap()
        );
    }
    assert!(!p.exists("cairn/items/.identity-migration.json"));
    p.expect(&["migrate"]);
}

#[test]
fn migration_recovery_refuses_intervening_edits_or_unsafe_paths() {
    let (p, _, _) = interrupted();
    p.append("cairn/items/CRN-0068-second.md", "A concurrent edit.\n");
    let before = p.read("cairn/items/CRN-0068-second.md");
    assert_contains(
        &p.fails(&["migrate"]).all(),
        "changed since",
        "never overwrite intervening edits",
    );
    assert_eq!(p.read("cairn/items/CRN-0068-second.md"), before);
    let (p, mut plan, _) = interrupted();
    plan["files"][0]["path"] = json!("../outside.md");
    p.write("cairn/items/.identity-migration.json", &plan.to_string());
    assert_contains(&p.fails(&["migrate"]).all(), "unsafe", "reject traversal");
}

#[test]
fn numeric_alias_and_uuid_prefix_namespace_collisions_are_errors() {
    let p = Project::new();
    let numeric_prefix = "12345678-0000-4000-8000-000000000001";
    uuid_item(&p, numeric_prefix, "First", "");
    uuid_item(&p, C, "Second", "");
    p.write(
        "cairn/items/_legacy-ids.toml",
        &format!("version = 1\nid_format = \"{{n:04}}\"\n[ids]\n12345678 = \"{C}\"\n"),
    );
    p.fails(&["show", "12345678"]);
    assert_eq!(full(&p, "123456780"), numeric_prefix);
    assert_eq!(
        p.json(&["show", numeric_prefix, "--json"])["ref"],
        "123456780"
    );
}

#[test]
fn migration_keeps_history_across_the_boundary_and_later_renames() {
    let p = legacy();
    git(&p, &["init", "-q"]);
    git(&p, &["config", "user.name", "Test"]);
    git(&p, &["config", "user.email", "test@example.com"]);
    git(&p, &["add", "."]);
    git(&p, &["commit", "-qm", "Before migration"]);
    let before = git(&p, &["rev-parse", "HEAD"]).trim().to_string();
    p.expect(&["migrate"]);
    git(&p, &["add", "."]);
    git(&p, &["commit", "-qm", "Migrate identities"]);
    let id = full(&p, "67");
    let range = p.json(&["log", "--range", &format!("{before}..HEAD"), "--json"]);
    assert!(
        range["items"].as_array().unwrap().is_empty(),
        "identity conversion is not new/removed work: {range}"
    );
    p.expect(&["set", "67", "title=Renamed"]);
    git(&p, &["add", "."]);
    git(&p, &["commit", "-qm", "Retitle"]);
    let history = p.json(&["log", &id, "--json"]);
    assert_eq!(
        history["revisions"].as_array().unwrap().len(),
        3,
        "{history}"
    );
    assert_eq!(p.json(&["log", "CRN-0067", "--json"]), history);
}

#[test]
fn import_export_preserves_uuid_identity_and_forward_references() {
    let p = Project::new();
    references(&p);
    p.write("import.json", &json!({"cairn":"2", "items":[
                {"id": A, "title":"First", "status":"done", "depends_on":[B], "fields":{"related":[B]}, "closed_at":"2026-01-02"},
        {"id": B, "title":"Second"}
    ]}).to_string());
    p.expect(&["import", "import.json"]);
    let first = p.json(&["show", A, "--json"]);
    assert_eq!(first["depends_on"], json!([B]));
    assert_eq!(first["fields"]["related"], json!([B]));
    assert_eq!(first["closed_at"], "2026-01-02");
    let export = p.json(&["export"]);
    assert_eq!(export["cairn"], "2");
    let receiver = Project::new();
    references(&receiver);
    receiver.write("import.json", &export.to_string());
    receiver.expect(&["import", "import.json"]);
    assert_eq!(full(&receiver, A), A);
    assert_eq!(full(&receiver, B), B);
    receiver.expect(&["import", "import.json", "--update"]);
    assert_eq!(receiver.count_all(), 2);
    assert_eq!(
        receiver.json(&["show", A, "--json"])["fields"]["related"],
        json!([B])
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

#[test]
fn independent_creation_from_identical_starting_projects_needs_no_allocator() {
    let left = Project::new();
    let right = Project::new();
    let a = left.add("Same title", &[]);
    let b = right.add("Same title", &[]);
    let a = full(&left, &a);
    let b = full(&right, &b);
    assert_ne!(a, b);
    let path = item_path(&right, &b);
    left.write(&path, &right.read(&path));
    left.expect(&["check", "--strict"]);
    assert_eq!(left.count_all(), 2);
}

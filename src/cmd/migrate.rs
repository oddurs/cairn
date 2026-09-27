// cairn — migrating a project between on-disk formats.
//
// Copyright (c) 2026 Oddur Sigurdsson. MIT licensed; see LICENSE.
//
// This command was written and tested while there was nothing to migrate,
// because a migration path invented at the moment it is first needed is a
// migration path nobody has tried. Format 2 then arrived and the scaffolding
// was already known to work, which is the whole argument for having done it.
//
// What a step reports matters as much as what it does: the question before
// running this over years of work is whether the item files are about to be
// rewritten, so every step answers that in files rather than in steps.
use crate::config::{CONFIG_FILE, CURRENT_FORMAT, Config, IdFormat};
use crate::identity::Id;
use crate::identity::{LEGACY_MAP, MIGRATION_JOURNAL};
use crate::lock::Lock;
use crate::store::Store;
use crate::style;
use anyhow::Result;
use anyhow::{Context, bail};
use clap::ArgAction;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(clap::Args)]
pub struct Args {
    /// Report what would change and write nothing
    #[arg(short = 'n', long, action = ArgAction::SetTrue)]
    pub dry_run: bool,

    /// Exit non-zero if the project is not already at the current format
    #[arg(long, action = ArgAction::SetTrue)]
    pub check: bool,

    /// Print nothing when there is nothing to do
    #[arg(short, long, action = ArgAction::SetTrue)]
    pub quiet: bool,
}

pub fn run(args: Args) -> Result<i32> {
    // Read leniently: every other command refuses a project behind the format,
    // which is what makes this command the only way forward.
    let cwd = std::env::current_dir()?;
    let Some(path) = Config::find(&cwd) else {
        anyhow::bail!("no {CONFIG_FILE} found in {} or any parent", cwd.display());
    };
    let cfg = Config::load_for_migration(&path)?;
    let from = cfg.format();

    if cfg.items_dir().join(MIGRATION_JOURNAL).exists() {
        if args.check {
            bail!("identity migration is unfinished; run `cairn migrate` to resume");
        }
        if args.dry_run {
            println!("resume the recorded identity migration; no new identities will be allocated");
            return Ok(0);
        }
        let _lock = Lock::acquire_for_migration(&cfg)?;
        let target = resume_identities(&cfg)?;
        println!(
            "{} resumed identity migration to format {target}",
            style::green("migrated:")
        );
        return Ok(0);
    }

    if from == CURRENT_FORMAT {
        if args.check {
            if !args.quiet {
                println!("{} format {from}, which is current", style::green("ok:"));
            }
            return Ok(0);
        }
        if !args.quiet {
            println!(
                "{} already at format {CURRENT_FORMAT}; nothing to migrate",
                style::green("ok:")
            );
        }
        return Ok(0);
    }

    if args.check {
        eprintln!(
            "{} project is format {from}, current is {CURRENT_FORMAT} — run `cairn migrate`",
            style::red("stale:")
        );
        return Ok(1);
    }

    let steps = plan(from, CURRENT_FORMAT);
    // Hold the migration lock before reading the data the plan will replace.
    let _lock = if args.dry_run {
        None
    } else {
        Some(Lock::acquire_for_migration(&cfg)?)
    };
    let store = Store::new(&cfg);
    let items = store.load_all()?;

    for (a, b) in &steps {
        let effect = effect_of(*a, *b, &cfg, &items);
        println!("  format {a} -> {b}  {}", style::dim(&effect.summary));
    }

    if args.dry_run {
        if steps.contains(&(4, 5)) {
            preview_numbers(&cfg, &items)?;
        }
        let total = steps
            .iter()
            .map(|(a, b)| effect_of(*a, *b, &cfg, &items))
            .fold(Effect::default(), Effect::and);
        print!("{}", report(&total));
        return Ok(0);
    }

    // The one command that may hold the lock on an older project.
    // A migration must never run against a backlog it cannot fully read; the
    // load above did that.
    for (from, to) in &steps {
        let step_cfg = Config::load_for_migration(&path)?;
        let step_items = Store::new(&step_cfg).load_all()?;
        apply(&step_cfg, &step_items, *from, *to)?;
        // The identity steps journal the configuration with everything else,
        // and write it last.
        if *to < 4 {
            stamp(&step_cfg, *to)?;
        }
    }

    println!(
        "{} {} item(s) now at format {CURRENT_FORMAT}",
        style::green("migrated:"),
        items.len()
    );
    Ok(0)
}

/// The chain of migrations between two formats.
///
/// Format 4 is a detour, not a stage: a format-3 project goes straight to 5,
/// gaining tags without ever trading its numbers for UUIDs.
fn plan(from: u32, to: u32) -> Vec<(u32, u32)> {
    let mut steps = Vec::new();
    let mut at = from;
    while at < to {
        let next = if at == 3 { 5 } else { at + 1 };
        steps.push((at, next.min(to)));
        at = next;
    }
    steps
}

fn apply(cfg: &Config, items: &[crate::item::Item], from: u32, to: u32) -> Result<()> {
    match (from, to) {
        (1, 2) => milestones_become_items(cfg, items),
        (2, 3) => types_declare_that_they_group(cfg),
        (3, 5) => tag_items(cfg, items),
        (4, 5) => restore_numbers(cfg, items),
        _ => anyhow::bail!("no migration is defined from format {from} to {to}"),
    }
}

/// Format 1 to 2: a milestone stops being a block in the configuration and
/// becomes an item.
///
/// No item file changes. `milestone: v0.1` in an item's frontmatter means
/// exactly what it meant before — which is the whole reason the reference is
/// addressed by key rather than by identifier, and why this touches one file
/// per project rather than every item.
fn milestones_become_items(cfg: &Config, items: &[crate::item::Item]) -> Result<()> {
    use crate::item::Item;
    use crate::refs::{MILESTONE_FIELD, MILESTONE_TYPE};

    let store = Store::new(cfg);
    // The order they were declared in becomes a dependency chain, because a
    // roadmap is a sequence and that is now how the order is expressed. The
    // rule that walked the list backwards so an undated milestone could inherit
    // the next dated one's date existed only because milestones had no natural
    // order; items have one.
    let mut previous: Option<Id> = None;
    let first = store.next_id(items)?.number().expect("legacy migration");
    for (offset, m) in cfg.milestones.iter().enumerate() {
        let next = Id::Num(
            first
                .checked_add(u32::try_from(offset)?)
                .ok_or_else(|| anyhow::anyhow!("legacy identifier space exhausted"))?,
        );
        let title = m.title.clone().unwrap_or_else(|| m.name.clone());
        let mut item = Item {
            id: next,
            meta: Default::default(),
            body: String::new(),
            path: store.path_for(next, None, &title),
            front: String::new(),
            eol: Default::default(),
        };
        item.meta.title = Some(title.clone());
        item.meta.key = Some(m.name.clone());
        item.meta.kind = Some(MILESTONE_TYPE.to_string());
        item.meta.status = Some(
            m.status
                .clone()
                .unwrap_or_else(|| cfg.initial_status().to_string()),
        );
        item.meta.created = Some(crate::store::today());
        if let Some(prev) = previous {
            item.meta.depends_on = vec![prev];
        }
        if let Some(due) = &m.due {
            item.set_extra("due", Some(crate::item::Field::Text(due.clone())));
        }
        // The description becomes the body, which is the point: the reason for
        // a date now lives with the date, and `cairn log` can say when it moved.
        if let Some(d) = &m.description {
            item.set_body(d);
        }
        store.stamp_new(&mut item)?;
        item.save()?;
        println!(
            "  {} {}  {title}",
            style::green("milestone"),
            style::bold(&cfg.format_id(next))
        );
        previous = Some(next);
    }

    // Then the configuration: the type and the field that names one, and the
    // blocks themselves removed.
    let path = cfg.root.join(CONFIG_FILE);
    let text = std::fs::read_to_string(&path)?;
    let mut doc: toml_edit::DocumentMut = text.parse()?;

    let has_type = doc
        .get("type")
        .and_then(|t| t.as_array_of_tables())
        .is_some_and(|a| {
            a.iter()
                .any(|t| t.get("name").and_then(|v| v.as_str()) == Some(MILESTONE_TYPE))
        });
    if !has_type {
        let mut table = toml_edit::Table::new();
        table["name"] = toml_edit::value(MILESTONE_TYPE);
        table["description"] = toml_edit::value("a release, or whatever this project ships");
        let types = doc
            .entry("type")
            .or_insert(toml_edit::Item::ArrayOfTables(Default::default()));
        if let Some(a) = types.as_array_of_tables_mut() {
            a.push(table);
        }
    }

    // `due` was a key on a [[milestone]] block; on an item it is an ordinary
    // custom field, and it has to be declared or `check` reports every
    // milestone the migration just wrote.
    let has_due = doc
        .get("field")
        .and_then(|t| t.as_array_of_tables())
        .is_some_and(|a| {
            a.iter()
                .any(|t| t.get("name").and_then(|v| v.as_str()) == Some("due"))
        });
    if !has_due {
        let mut table = toml_edit::Table::new();
        table["name"] = toml_edit::value("due");
        table["kind"] = toml_edit::value("date");
        table["description"] = toml_edit::value("when a milestone is meant to land");
        let fields = doc
            .entry("field")
            .or_insert(toml_edit::Item::ArrayOfTables(Default::default()));
        if let Some(a) = fields.as_array_of_tables_mut() {
            a.push(table);
        }
    }

    let has_field = doc
        .get("field")
        .and_then(|t| t.as_array_of_tables())
        .is_some_and(|a| {
            a.iter()
                .any(|t| t.get("name").and_then(|v| v.as_str()) == Some(MILESTONE_FIELD))
        });
    if !has_field {
        let mut table = toml_edit::Table::new();
        table["name"] = toml_edit::value(MILESTONE_FIELD);
        table["kind"] = toml_edit::value("ref");
        table["target"] = toml_edit::value(MILESTONE_TYPE);
        table["by"] = toml_edit::value("key");
        table["rollup"] = toml_edit::value(true);
        table["inverse"] = toml_edit::value("scheduled");
        table["description"] = toml_edit::value("what this ships in");
        let fields = doc
            .entry("field")
            .or_insert(toml_edit::Item::ArrayOfTables(Default::default()));
        if let Some(a) = fields.as_array_of_tables_mut() {
            a.push(table);
        }
    }

    doc.remove("milestone");
    crate::store::write_atomic(&path, doc.to_string().as_bytes())?;
    println!(
        "  {} [[milestone]] replaced by a type and a reference",
        style::green("cairn.toml")
    );
    Ok(())
}

/// Record the new format in cairn.toml, preserving its comments.
fn stamp(cfg: &Config, format: u32) -> Result<()> {
    let path = cfg.root.join(CONFIG_FILE);
    let text = std::fs::read_to_string(&path)?;
    let mut doc: toml_edit::DocumentMut = text.parse()?;
    doc["format"] = toml_edit::value(i64::from(format));
    crate::store::write_atomic(&path, doc.to_string().as_bytes())
}

/// Persist the complete before/after image before replacing anything. The
/// configuration is the last replacement: no reader may see the new format
/// with a partially converted backlog. A journal survives a crash until every
/// file has been flushed, and is never silently discarded on conflicting edits.
///
/// Version 1 journals were written by the format-4 migration and are still
/// resumed; version 2 adds removals, which is how a rename is recorded.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IdentityPlan {
    version: u32,
    #[serde(default = "format_four")]
    target: u32,
    files: Vec<Replacement>,
}

fn format_four() -> u32 {
    4
}

/// One file's journey. `before` is what is there now, or nothing; `after` is
/// what will be, or nothing, which removes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Replacement {
    path: String,
    before: Option<String>,
    after: Option<String>,
}

/// Format 3 to 5: every item gains a `uid` tag, and nothing else about it
/// changes — not its number, its references, its filename or a byte of its
/// body. The line is inserted beside `id:` rather than the frontmatter being
/// rewritten, so the diff is one line per item.
fn tag_items(cfg: &Config, items: &[crate::item::Item]) -> Result<()> {
    let mut target = cfg.clone();
    target.format = Some(5);
    refuse_keys_that_read_as_ids(&target, items)?;
    let store = Store::new(cfg);
    let mut files = Vec::new();
    let mut tags = BTreeSet::new();
    for item in items {
        let before = std::fs::read_to_string(&item.path)?;
        // An item an earlier step just wrote is tagged already, and is
        // journaled unchanged so the journal still accounts for every file.
        if item.meta.uid.is_some() {
            files.push(Replacement {
                path: store.rel(&item.path),
                after: Some(before.clone()),
                before: Some(before),
            });
            continue;
        }
        let uid = loop {
            let uid = crate::identity::new_uid()?;
            if tags.insert(uid) {
                break uid;
            }
        };
        files.push(Replacement {
            path: store.rel(&item.path),
            after: Some(insert_uid(&before, uid)?),
            before: Some(before),
        });
    }
    files.push(config_replacement(cfg, 5, None)?);
    journal_and_apply(
        cfg,
        IdentityPlan {
            version: 2,
            target: 5,
            files,
        },
    )
}

/// The `uid:` line, after `id:` when there is one and first otherwise, in the
/// file's own line ending.
fn insert_uid(original: &str, uid: uuid::Uuid) -> Result<String> {
    let eol = if original.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let opening = original
        .find('\n')
        .context("missing frontmatter opening line")?
        + 1;
    let mut at = opening;
    for line in original[opening..].split_inclusive('\n') {
        let bare = line.trim_end_matches(['\r', '\n']);
        if matches!(bare, "---" | "...") {
            break;
        }
        if bare.starts_with("id:") {
            at += line.len();
            return Ok(format!(
                "{}uid: {uid}{eol}{}",
                &original[..at],
                &original[at..]
            ));
        }
        at += line.len();
    }
    Ok(format!(
        "{}uid: {uid}{eol}{}",
        &original[..opening],
        &original[opening..]
    ))
}

/// Format 4 to 5: every item's number comes back, and its UUID becomes its tag.
///
/// An item that had a number before format 4 gets exactly that number, read
/// from `_legacy-ids.toml`. One created since gets the next free number, in the
/// order the items were created, after every number the map, this directory or
/// any branch has used. Every id reference is rewritten from UUID to number, a
/// UUID-named file is renamed to its number, the project's old `id_format`
/// returns to `cairn.toml`, and the frozen map, which nothing needs any more,
/// is removed. Bodies are not touched.
fn restore_numbers(cfg: &Config, items: &[crate::item::Item]) -> Result<()> {
    let store = Store::new(cfg);
    let report = crate::cmd::check::collect(cfg, &store)?;
    if !report.errors.is_empty() {
        bail!(
            "repair the backlog before migrating:\n{}",
            report.errors.join("\n")
        );
    }
    let legacy = cfg.identities.borrow().legacy.clone();
    let mut target = cfg.clone();
    target.format = Some(5);
    let restored_format = IdFormat::compile(&legacy.id_format)?;
    let template = (restored_format != IdFormat::padded(cfg.project.id_width))
        .then(|| legacy.id_format.clone());
    target.project.id_format.clone_from(&template);
    refuse_keys_that_read_as_ids(&target, items)?;

    let numbers = numbers_for(cfg, items)?;

    let number_of = |id: Id| -> Result<Id> {
        match id {
            Id::Uuid(u) => numbers
                .get(&u)
                .map(|n| Id::Num(*n))
                .ok_or_else(|| anyhow::anyhow!("unresolved reference {u}")),
            Id::Num(_) => Ok(id),
        }
    };
    let mut writes = Vec::new();
    let mut removals = Vec::new();
    let mut destinations = BTreeSet::new();
    for item in items {
        let Id::Uuid(uid) = item.id else {
            bail!("{} has no UUID in a format-4 project", item.path.display());
        };
        let id = number_of(item.id)?;
        let before = std::fs::read_to_string(&item.path)?;
        let original: serde_yaml_ng::Mapping = serde_yaml_ng::from_str(&item.front)?;
        let mut front = serde_yaml_ng::Mapping::new();
        for (key, value) in original {
            let mut value = value;
            match key.as_str() {
                Some("id") => {
                    front.insert(key, id.yaml());
                    front.insert("uid".into(), serde_yaml_ng::Value::String(uid.to_string()));
                    continue;
                }
                Some("depends_on") => {
                    value = serde_yaml_ng::Value::Sequence(
                        item.meta
                            .depends_on
                            .iter()
                            .map(|d| number_of(*d).map(Id::yaml))
                            .collect::<Result<_>>()
                            .with_context(|| format!("{}: depends_on", item.path.display()))?,
                    );
                }
                Some(name)
                    if cfg
                        .all_ref_fields()
                        .iter()
                        .any(|f| f.name == name && f.by == crate::config::Addressing::Id) =>
                {
                    number_references(&mut value, &number_of)
                        .with_context(|| format!("{}: {name}", item.path.display()))?;
                }
                _ => {}
            }
            front.insert(key, value);
        }
        let after = replace_frontmatter(&before, &front)?;
        let parent = item.path.parent().context("item has no directory")?;
        // Named as format 3 named it. A file that kept its old number
        // through format 4 is already right, and keeps its history unbroken.
        let destination = parent.join(target.filename_for(id, item.kind(), item.title()));
        if !destinations.insert(destination.clone()) {
            bail!("two items would be named {}", destination.display());
        }
        if destination == item.path {
            writes.push(Replacement {
                path: store.rel(&item.path),
                before: Some(before),
                after: Some(after),
            });
        } else {
            if destination.exists() {
                bail!(
                    "{} is in the way of {}; move it aside before migrating",
                    destination.display(),
                    item.path.display()
                );
            }
            writes.push(Replacement {
                path: store.rel(&destination),
                before: None,
                after: Some(after),
            });
            removals.push(Replacement {
                path: store.rel(&item.path),
                before: Some(before),
                after: None,
            });
        }
    }
    let map_path = cfg.items_dir().join(LEGACY_MAP);
    if let Some(text) = read_optional(&map_path)? {
        removals.push(Replacement {
            path: store.rel(&map_path),
            before: Some(text),
            after: None,
        });
    }
    let mut files = writes;
    files.extend(removals);
    files.push(config_replacement(cfg, 5, template.as_deref())?);
    journal_and_apply(
        cfg,
        IdentityPlan {
            version: 2,
            target: 5,
            files,
        },
    )
}

/// The number each format-4 item gets: the one it had, from the map, or for an
/// item created since, the next free one in the order they were created.
///
/// Every number the map holds is spoken for, including those whose items have
/// since been removed: a number is never handed to different work. So is every
/// number any branch has used, through the ordinary allocator.
fn numbers_for(cfg: &Config, items: &[crate::item::Item]) -> Result<BTreeMap<uuid::Uuid, u32>> {
    let store = Store::new(cfg);
    let legacy = cfg.identities.borrow().legacy.clone();
    let mut numbers: BTreeMap<uuid::Uuid, u32> = BTreeMap::new();
    for (number, id) in &legacy.ids {
        let n: u32 = number.parse()?;
        cfg.remember_id(Id::Num(n));
        if let Id::Uuid(uid) = id {
            numbers.insert(*uid, n);
        }
    }
    let mut fresh: Vec<&crate::item::Item> = items
        .iter()
        .filter(|i| matches!(i.id, Id::Uuid(u) if !numbers.contains_key(&u)))
        .collect();
    fresh.sort_by(|a, b| {
        a.meta
            .created
            .cmp(&b.meta.created)
            .then_with(|| a.id.cmp(&b.id))
    });
    for item in fresh {
        let Id::Uuid(uid) = item.id else { continue };
        let Id::Num(n) = store.next_id(&[])? else {
            bail!("the allocator returned a UUID");
        };
        numbers.insert(uid, n);
    }
    Ok(numbers)
}

/// What a dry run shows for format 4: the numbers items without one will get,
/// which is the one thing about the migration nobody can work out beforehand.
fn preview_numbers(cfg: &Config, items: &[crate::item::Item]) -> Result<()> {
    let restored = cfg.identities.borrow().legacy.clone();
    let template = IdFormat::compile(&restored.id_format)?;
    let numbers = numbers_for(cfg, items)?;
    let mut fresh: Vec<(u32, &crate::item::Item)> = items
        .iter()
        .filter_map(|i| match i.id {
            Id::Uuid(u) if !restored.ids.values().any(|v| *v == i.id) => {
                numbers.get(&u).map(|n| (*n, i))
            }
            _ => None,
        })
        .collect();
    fresh.sort_by_key(|(n, _)| *n);
    println!(
        "\n  {} item(s) get back the number they had; {} created since are numbered:",
        items.len() - fresh.len(),
        fresh.len()
    );
    for (n, item) in fresh {
        println!(
            "    {} {} {}  {}",
            style::dim(&item.id.compact()[..8]),
            style::dim("->"),
            style::bold(&template.render(n)),
            item.title()
        );
    }
    Ok(())
}

/// A key a reference names must not read as an id under the format arriving,
/// or `milestone: cafe0042` could name either.
fn refuse_keys_that_read_as_ids(target: &Config, items: &[crate::item::Item]) -> Result<()> {
    for item in items {
        if let Some(key) = item.key()
            && target.looks_like_id(key)
        {
            bail!(
                "key `{key}` reads as an identifier in format {}; rename it and its key \
                 references before migrating",
                target.format()
            );
        }
    }
    Ok(())
}

/// The configuration at its new format, written last.
fn config_replacement(cfg: &Config, format: u32, id_format: Option<&str>) -> Result<Replacement> {
    let before = std::fs::read_to_string(cfg.root.join(CONFIG_FILE))?;
    let mut doc: toml_edit::DocumentMut = before.parse()?;
    doc["format"] = toml_edit::value(i64::from(format));
    if let Some(template) = id_format {
        doc["project"]["id_format"] = toml_edit::value(template);
    }
    Ok(Replacement {
        path: CONFIG_FILE.into(),
        before: Some(before),
        after: Some(doc.to_string()),
    })
}

fn journal_and_apply(cfg: &Config, plan: IdentityPlan) -> Result<()> {
    validate_plan(cfg, &plan)?;
    crate::store::write_atomic(
        &cfg.items_dir().join(MIGRATION_JOURNAL),
        &serde_json::to_vec_pretty(&plan)?,
    )?;
    sync_directory(&cfg.items_dir())?;
    resume_identities(cfg).map(|_| ())
}

fn number_references(
    value: &mut serde_yaml_ng::Value,
    number_of: &dyn Fn(Id) -> Result<Id>,
) -> Result<()> {
    use serde_yaml_ng::Value;
    match value {
        Value::Null => (),
        Value::Sequence(values) => {
            for value in values {
                number_references(value, number_of)?;
            }
        }
        _ => {
            let raw = match value {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => bail!("reference is not an identifier"),
            };
            *value = number_of(raw.parse::<Id>()?)?.yaml();
        }
    }
    Ok(())
}

/// Only replace YAML. Preserve the opening line, closing delimiter and every
/// byte following it, including CRLF, blank lines and trailing whitespace.
fn replace_frontmatter(original: &str, front: &serde_yaml_ng::Mapping) -> Result<String> {
    let opening = original
        .find('\n')
        .context("missing frontmatter opening line")?
        + 1;
    let mut closing = opening;
    for line in original[opening..].split_inclusive('\n') {
        if matches!(line.trim_end_matches(['\r', '\n']), "---" | "...") {
            let yaml = serde_yaml_ng::to_string(front)?;
            let yaml = if original[..opening].ends_with("\r\n") {
                yaml.replace('\n', "\r\n")
            } else {
                yaml
            };
            return Ok(format!(
                "{}{}{}",
                &original[..opening],
                yaml,
                &original[closing..]
            ));
        }
        closing += line.len();
    }
    bail!("missing frontmatter closing delimiter")
}

fn read_optional(path: &std::path::Path) -> Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

fn validate_plan(cfg: &Config, plan: &IdentityPlan) -> Result<()> {
    use std::path::{Component, Path};
    if !matches!(plan.version, 1 | 2)
        || plan
            .files
            .last()
            .is_none_or(|f| f.path != CONFIG_FILE || f.after.is_none())
    {
        bail!("invalid identity migration journal: version or final configuration replacement");
    }
    let mut paths = BTreeSet::new();
    for file in &plan.files {
        let relative = Path::new(&file.path);
        if relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
            || !paths.insert(file.path.clone())
        {
            bail!("unsafe or duplicate migration path: {}", file.path);
        }
        let path = cfg.root.join(relative);
        if file.path != CONFIG_FILE
            && !(path.starts_with(cfg.items_dir())
                && (path == cfg.items_dir().join(LEGACY_MAP)
                    || path
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("md"))))
        {
            bail!("migration path is outside the item store: {}", file.path);
        }
        let mut ancestor = path.as_path();
        while ancestor != cfg.root {
            if std::fs::symlink_metadata(ancestor).is_ok_and(|m| m.file_type().is_symlink()) {
                bail!("migration refuses symbolic link {}", ancestor.display());
            }
            ancestor = ancestor
                .parent()
                .context("migration path has no project parent")?;
        }
        let current = read_optional(&path)?;
        if current != file.after && current != file.before {
            bail!(
                "{} changed since migration was planned; preserve that edit and reconcile it with {} before resuming",
                file.path,
                MIGRATION_JOURNAL
            );
        }
    }
    let after: Config = toml::from_str(
        plan.files
            .last()
            .and_then(|f| f.after.as_deref())
            .unwrap_or_default(),
    )?;
    if after.format() != plan.target || after.project.dir != cfg.project.dir {
        bail!("invalid identity migration configuration target");
    }
    // Listed rather than loaded: halfway through, no one format's reader can
    // read every file, and that is exactly when this runs.
    for path in crate::store::item_paths(&cfg.items_dir())? {
        if !paths.contains(&Store::new(cfg).rel(&path)) {
            bail!(
                "{} was added during migration; preserve it outside the item directory before resuming",
                path.display()
            );
        }
    }
    Ok(())
}

/// Finish what the journal records, and say which format it was going to.
fn resume_identities(cfg: &Config) -> Result<u32> {
    let journal = cfg.items_dir().join(MIGRATION_JOURNAL);
    let plan: IdentityPlan = serde_json::from_str(&std::fs::read_to_string(&journal)?)
        .context("reading identity migration journal")?;
    validate_plan(cfg, &plan)?;
    for file in &plan.files {
        let path = cfg.root.join(&file.path);
        let current = read_optional(&path)?;
        if current == file.after {
            continue;
        }
        if current != file.before {
            bail!(
                "{} changed during migration; refusing to overwrite it",
                file.path
            );
        }
        match &file.after {
            Some(text) => crate::store::write_atomic(&path, text.as_bytes())?,
            None => std::fs::remove_file(&path)
                .with_context(|| format!("removing {}", path.display()))?,
        }
        sync_directory(path.parent().context("migration file has no parent")?)?;
    }
    std::fs::remove_file(&journal)?;
    sync_directory(&cfg.items_dir())?;
    Ok(plan.target)
}

fn sync_directory(path: &std::path::Path) -> Result<()> {
    #[cfg(unix)]
    std::fs::File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// What a migration step will touch.
///
/// The step count `--dry-run` used to print is the one thing nobody wants to
/// know. The question before running a migration over years of work is what it
/// will change, and specifically whether the item files are about to be
/// rewritten — so each step answers that, and the dry run reports it in files
/// rather than in steps.
#[derive(Default)]
struct Effect {
    /// Whole files replaced, named.
    rewritten: Vec<String>,
    /// New items written.
    created: usize,
    /// Existing items rewritten, named. Empty is the good case and the common
    /// one, and saying so plainly is the point of all this.
    modified: Vec<String>,
    /// Files deleted, named.
    removed: Vec<String>,
    summary: String,
}

impl Effect {
    fn and(mut self, other: Effect) -> Effect {
        // A file several steps rewrite is still one file to restore.
        for f in other.rewritten {
            if !self.rewritten.contains(&f) {
                self.rewritten.push(f);
            }
        }
        self.created += other.created;
        self.modified.extend(other.modified);
        self.removed.extend(other.removed);
        self
    }
}

/// What `--dry-run` prints, as a string, so that both branches are testable —
/// including the one no migration has needed yet.
fn report(total: &Effect) -> String {
    let mut out = String::from("\n");
    for f in &total.rewritten {
        out += &format!("  {:<10} {f}\n", style::dim("rewritten"));
    }
    for f in &total.removed {
        out += &format!("  {:<10} {f}\n", style::dim("removed"));
    }
    out += &format!(
        "  {:<10} {} new item(s)\n",
        style::dim("created"),
        total.created
    );
    // The sentence somebody actually wants before running this on years of
    // work, printed only when it is true.
    if total.modified.is_empty() && total.removed.is_empty() {
        out += &format!(
            "\n{}\n",
            style::green("nothing already in the item directory will be changed.")
        );
        // And therefore what it takes to go back, which is the other half of the
        // question somebody asks before running this on years of work. Said only
        // where it is true: a migration that rewrote existing items would not be
        // undone this cheaply, and the branch below says so instead.
        out += &format!(
            "{}\n",
            style::dim(&match total.created {
                0 => format!(
                    "to undo it, restore {} — nothing else changes.",
                    crate::config::CONFIG_FILE
                ),
                n => format!(
                    "to undo it, restore {} and remove the {n} item(s) it creates.",
                    crate::config::CONFIG_FILE
                ),
            })
        );
    } else {
        out += &format!(
            "\n{} {} existing item(s) would be rewritten:\n",
            style::yellow("careful:"),
            total.modified.len()
        );
        for f in &total.modified {
            out += &format!("    {f}\n");
        }
    }
    out
}

fn effect_of(from: u32, to: u32, cfg: &Config, items: &[crate::item::Item]) -> Effect {
    match (from, to) {
        (1, 2) => Effect {
            rewritten: vec![CONFIG_FILE.to_string()],
            created: cfg.milestones.len(),
            summary: "milestones move from cairn.toml into items".into(),
            ..Default::default()
        },
        (2, 3) => Effect {
            rewritten: vec![CONFIG_FILE.to_string()],
            summary: "a type declares that it groups work".into(),
            ..Default::default()
        },
        (3, 5) => Effect {
            rewritten: vec![CONFIG_FILE.to_string()],
            modified: items.iter().map(|i| Store::new(cfg).rel(&i.path)).collect(),
            summary: "every item gains a `uid` tag; numbers, references, filenames and bodies are unchanged".into(),
            ..Default::default()
        },
        (4, 5) => Effect {
            rewritten: vec![CONFIG_FILE.to_string()],
            removed: std::iter::once(cfg.items_dir().join(LEGACY_MAP))
                .filter(|p| p.exists())
                .map(|p| Store::new(cfg).rel(&p))
                .collect(),
            modified: items.iter().map(|i| Store::new(cfg).rel(&i.path)).collect(),
            summary: "numbers return, restored from _legacy-ids.toml; each UUID becomes the item's `uid`; references and UUID-named files are renumbered".into(),
            ..Default::default()
        },
        _ => Effect {
            summary: "unknown step".into(),
            ..Default::default()
        },
    }
}

/// Format 2 to 3: container-ness moves from a field's `target` onto the type.
///
/// A `[[field]]` of `kind = "ref"` naming a specific type was the only way to
/// say that a type groups work, which meant the fact lived at the wrong end —
/// unreadable from the type, and changed by editing an unrelated field. Now the
/// type says it, and the field it implies is deleted.
///
/// **No item file changes.** `milestone: v0.1` means exactly what it meant, for
/// the same reason format 2 could move milestones into items without touching
/// one: what the key names did not change, only how the schema says so.
fn types_declare_that_they_group(cfg: &Config) -> Result<()> {
    let path = cfg.root.join(CONFIG_FILE);
    let text = std::fs::read_to_string(&path)?;
    let mut doc: toml_edit::DocumentMut = text.parse()?;

    // Which types some ref field pointed at, and how.
    let mut grouping: Vec<(String, &'static str, Option<String>)> = Vec::new();
    if let Some(fields) = doc.get("field").and_then(|f| f.as_array_of_tables()) {
        for f in fields {
            if f.get("kind").and_then(|v| v.as_str()) != Some("ref") {
                continue;
            }
            let Some(target) = f.get("target").and_then(|v| v.as_str()) else {
                continue;
            };
            if target == "*" {
                continue; // a general reference, which stays a field
            }
            let many = f.get("cardinality").and_then(|v| v.as_str()) == Some("many");
            let inverse = f
                .get("inverse")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            grouping.push((
                target.to_string(),
                if many { "many" } else { "one" },
                inverse,
            ));
        }
    }

    // Say it on the type.
    if let Some(types) = doc.get_mut("type").and_then(|t| t.as_array_of_tables_mut()) {
        for table in types.iter_mut() {
            let Some(name) = table
                .get("name")
                .and_then(|v| v.as_str())
                .map(str::to_string)
            else {
                continue;
            };
            if let Some((_, how, inverse)) = grouping.iter().find(|(t, _, _)| *t == name) {
                table["groups"] = toml_edit::value(*how);
                if let Some(i) = inverse {
                    table["inverse"] = toml_edit::value(i.as_str());
                }
                println!(
                    "  {} {name} groups work — {how} per item",
                    style::dim("type")
                );
            }
        }
    }

    // And delete the field that said it, since the type now does.
    if let Some(fields) = doc
        .get_mut("field")
        .and_then(|f| f.as_array_of_tables_mut())
    {
        let named: Vec<String> = grouping.iter().map(|(t, _, _)| t.clone()).collect();
        fields.retain(|f| {
            let is_grouping_ref = f.get("kind").and_then(|v| v.as_str()) == Some("ref")
                && f.get("target")
                    .and_then(|v| v.as_str())
                    .is_some_and(|t| named.iter().any(|n| n == t));
            if is_grouping_ref && let Some(n) = f.get("name").and_then(|v| v.as_str()) {
                println!(
                    "  {} [[field]] {n}, now implied by its type",
                    style::dim("removed")
                );
            }
            !is_grouping_ref
        });
    }

    crate::store::write_atomic(&path, doc.to_string().as_bytes())?;
    println!("  {} {CONFIG_FILE}", style::dim("rewritten"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The good case, and the one every migration so far has been.
    #[test]
    fn a_migration_that_touches_no_item_says_so() {
        let out = report(&Effect {
            rewritten: vec![CONFIG_FILE.to_string()],
            created: 2,
            modified: Vec::new(),
            summary: String::new(),
            ..Default::default()
        });
        assert!(out.contains("rewritten"), "{out}");
        assert!(out.contains("2 new item(s)"), "{out}");
        assert!(
            out.contains("nothing already in the item directory will be changed."),
            "{out}"
        );
        // And what it takes to go back, which is the other half of the question
        // somebody asks before running this on years of work. The count matters:
        // a migration that creates items needs them removed as well.
        assert!(out.contains("to undo it"), "{out}");
        assert!(out.contains("remove the 2 item(s)"), "{out}");
        assert!(!out.contains("careful"), "{out}");

        // With nothing created, restoring the one file is the whole of it.
        let out = report(&Effect {
            rewritten: vec![CONFIG_FILE.to_string()],
            created: 0,
            modified: Vec::new(),
            summary: String::new(),
            ..Default::default()
        });
        assert!(out.contains("nothing else changes"), "{out}");
        assert!(!out.contains("remove the"), "{out}");
    }

    /// No migration has needed to rewrite an item yet. The day one does, the
    /// person running it must be told which files, by name, before it happens —
    /// so the branch is written and held to account now rather than then.
    #[test]
    fn a_migration_that_rewrites_items_names_them() {
        let out = report(&Effect {
            rewritten: vec![CONFIG_FILE.to_string()],
            created: 0,
            modified: vec!["0001-a.md".into(), "0002-b.md".into()],
            summary: String::new(),
            ..Default::default()
        });
        assert!(out.contains("careful:"), "{out}");
        assert!(
            out.contains("2 existing item(s) would be rewritten"),
            "{out}"
        );
        assert!(out.contains("0001-a.md"), "{out}");
        assert!(out.contains("0002-b.md"), "{out}");
        assert!(
            !out.contains("nothing already in the item directory"),
            "the reassuring sentence must not appear when it is false: {out}"
        );
    }
}

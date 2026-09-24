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
use crate::config::{CONFIG_FILE, CURRENT_FORMAT, Config};
use crate::identity::Id;
use crate::identity::{LEGACY_MAP, LegacyMap, MIGRATION_JOURNAL};
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
        resume_identities(&cfg)?;
        println!(
            "{} resumed identity migration to format 4",
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

/// The chain of single-step migrations between two formats.
fn plan(from: u32, to: u32) -> Vec<(u32, u32)> {
    (from..to).map(|n| (n, n + 1)).collect()
}

fn apply(cfg: &Config, items: &[crate::item::Item], from: u32, to: u32) -> Result<()> {
    match (from, to) {
        (1, 2) => milestones_become_items(cfg, items),
        (2, 3) => types_declare_that_they_group(cfg),
        (3, 4) => migrate_identities(cfg, items),
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
    let first = store.next_id(items)?.legacy().expect("legacy migration");
    for (offset, m) in cfg.milestones.iter().enumerate() {
        let next = Id::Legacy(
            first
                .checked_add(u32::try_from(offset)?)
                .ok_or_else(|| anyhow::anyhow!("legacy identifier space exhausted"))?,
        );
        let title = m.title.clone().unwrap_or_else(|| m.name.clone());
        let mut item = Item {
            id: next,
            meta: Default::default(),
            body: String::new(),
            path: store.path_for(next, &title),
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
/// configuration is the last replacement: no reader may see format 4 with a
/// partially converted backlog. A journal survives a crash until every file
/// has been flushed, and is never silently discarded on conflicting edits.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IdentityPlan {
    version: u32,
    files: Vec<Replacement>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Replacement {
    path: String,
    before: Option<String>,
    after: String,
}

fn migrate_identities(cfg: &Config, items: &[crate::item::Item]) -> Result<()> {
    let mut target = cfg.clone();
    target.format = Some(4);
    for item in items {
        if let Some(key) = item.key()
            && target.looks_like_id(key)
        {
            bail!(
                "key `{key}` is reserved for identifiers in format 4; rename it and its key references before migrating"
            );
        }
    }
    let report = crate::cmd::check::collect(cfg, &Store::new(cfg))?;
    if !report.errors.is_empty() {
        bail!(
            "repair the legacy backlog before migrating:\n{}",
            report.errors.join("\n")
        );
    }
    let mut ids = BTreeMap::new();
    let mut allocated = BTreeSet::new();
    for item in items {
        let Some(n) = item.id.legacy() else {
            bail!(
                "{} already has a UUID in a legacy project",
                item.path.display()
            );
        };
        let id = loop {
            let candidate = Id::new()?;
            if allocated.insert(candidate) {
                break candidate;
            }
        };
        if ids.insert(n.to_string(), id).is_some() {
            bail!("legacy id {n} occurs more than once; repair it before migrating");
        }
    }
    let map = LegacyMap {
        version: 1,
        id_format: cfg.project.id_format.clone().unwrap_or_else(|| {
            if cfg.project.id_width == 0 {
                "{n}".into()
            } else {
                format!("{{n:0{}}}", cfg.project.id_width)
            }
        }),
        ids,
    };
    map.validate()?;
    let mut files = Vec::new();
    for item in items {
        let before = std::fs::read_to_string(&item.path)?;
        let mut front: serde_yaml_ng::Mapping = serde_yaml_ng::from_str(&item.front)?;
        front.insert("id".into(), map.canonical(item.id).yaml());
        for field in cfg
            .all_ref_fields()
            .into_iter()
            .filter(|f| f.by == crate::config::Addressing::Id)
        {
            if let Some(value) = front.get_mut(serde_yaml_ng::Value::String(field.name.clone())) {
                // Historical depends_on accepts comma-separated scalar input.
                // Use its already-parsed meaning before mapping identities.
                if field.name == "depends_on" {
                    *value = serde_yaml_ng::Value::Sequence(
                        item.meta
                            .depends_on
                            .iter()
                            .map(|id| map.canonical(*id).yaml())
                            .collect(),
                    );
                    continue;
                }
                rewrite_reference(value, &map)
                    .with_context(|| format!("{}: {}", item.path.display(), field.name))?;
            }
        }
        let after = replace_frontmatter(&before, &front)?;
        files.push(Replacement {
            path: Store::new(cfg).rel(&item.path),
            before: Some(before),
            after,
        });
    }
    let map_path = cfg.items_dir().join(LEGACY_MAP);
    if map_path.exists() {
        bail!(
            "{} already exists; refusing to overwrite an identity map",
            map_path.display()
        );
    }
    files.push(Replacement {
        path: Store::new(cfg).rel(&map_path),
        before: None,
        after: toml::to_string_pretty(&map)?,
    });
    let before = std::fs::read_to_string(cfg.root.join(CONFIG_FILE))?;
    let mut doc: toml_edit::DocumentMut = before.parse()?;
    doc["format"] = toml_edit::value(4);
    if let Some(project) = doc
        .get_mut("project")
        .and_then(toml_edit::Item::as_table_like_mut)
    {
        for field in ["id_format", "id_width", "id_start"] {
            project.remove(field);
        }
    }
    files.push(Replacement {
        path: CONFIG_FILE.into(),
        before: Some(before),
        after: doc.to_string(),
    });
    let plan = IdentityPlan { version: 1, files };
    validate_plan(cfg, &plan)?;
    crate::store::write_atomic(
        &cfg.items_dir().join(MIGRATION_JOURNAL),
        &serde_json::to_vec_pretty(&plan)?,
    )?;
    sync_directory(&cfg.items_dir())?;
    resume_identities(cfg)
}

fn rewrite_reference(value: &mut serde_yaml_ng::Value, map: &LegacyMap) -> Result<()> {
    use serde_yaml_ng::Value;
    match value {
        Value::Null => (),
        Value::Sequence(values) => {
            for value in values {
                rewrite_reference(value, map)?;
            }
        }
        _ => {
            let raw = match value {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => bail!("reference is not an identifier"),
            };
            let Some(id) = map.resolve(&raw) else {
                bail!("unresolved legacy reference `{raw}`");
            };
            *value = id.yaml();
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
    if plan.version != 1 || plan.files.last().is_none_or(|f| f.path != CONFIG_FILE) {
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
        if current.as_deref() != Some(file.after.as_str()) && current != file.before {
            bail!(
                "{} changed since migration was planned; preserve that edit and reconcile it with {} before resuming",
                file.path,
                MIGRATION_JOURNAL
            );
        }
    }
    let after: Config = toml::from_str(&plan.files.last().unwrap().after)?;
    if after.format() != 4 || after.project.dir != cfg.project.dir {
        bail!("invalid identity migration configuration target");
    }
    let items = Store::new(cfg).load_all()?;
    for item in items {
        if !paths.contains(&Store::new(cfg).rel(&item.path)) {
            bail!(
                "{} was added during migration; preserve it outside the item directory before resuming",
                item.path.display()
            );
        }
    }
    Ok(())
}

fn resume_identities(cfg: &Config) -> Result<()> {
    let journal = cfg.items_dir().join(MIGRATION_JOURNAL);
    let plan: IdentityPlan = serde_json::from_str(&std::fs::read_to_string(&journal)?)
        .context("reading identity migration journal")?;
    validate_plan(cfg, &plan)?;
    for file in &plan.files {
        let path = cfg.root.join(&file.path);
        let current = read_optional(&path)?;
        if current.as_deref() == Some(&file.after) {
            continue;
        }
        if current != file.before {
            bail!(
                "{} changed during migration; refusing to overwrite it",
                file.path
            );
        }
        crate::store::write_atomic(&path, file.after.as_bytes())?;
        sync_directory(path.parent().context("migration file has no parent")?)?;
    }
    std::fs::remove_file(&journal)?;
    sync_directory(&cfg.items_dir())
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
    summary: String,
}

impl Effect {
    fn and(mut self, other: Effect) -> Effect {
        self.rewritten.extend(other.rewritten);
        self.created += other.created;
        self.modified.extend(other.modified);
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
    out += &format!(
        "  {:<10} {} new item(s)\n",
        style::dim("created"),
        total.created
    );
    // The sentence somebody actually wants before running this on years of
    // work, printed only when it is true.
    if total.modified.is_empty() {
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
            modified: Vec::new(),
            summary: "milestones move from cairn.toml into items".into(),
        },
        (2, 3) => Effect {
            rewritten: vec![CONFIG_FILE.to_string()],
            created: 0,
            modified: Vec::new(),
            summary: "a type declares that it groups work".into(),
        },
        (3, 4) => Effect {
            rewritten: vec![CONFIG_FILE.to_string(), Store::new(cfg).rel(&cfg.items_dir().join(LEGACY_MAP))],
            modified: items.iter().map(|i| Store::new(cfg).rel(&i.path)).collect(),
            summary: "immutable UUID identities and references; preserve legacy lookup, filenames, bodies and history".into(),
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

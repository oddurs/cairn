// cairn — migrating a project between on-disk formats.
//
// Copyright (C) 2026 Oddur Sigurdsson
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, either version 3 of the License, or (at your option) any later
// version.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along with
// this program.  If not, see <https://www.gnu.org/licenses/>.
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
use crate::lock::Lock;
use crate::store::Store;
use crate::style;
use anyhow::Result;
use clap::ArgAction;

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
    let _lock = Lock::acquire_for_migration(&cfg)?;
    for (from, to) in &steps {
        apply(&cfg, &items, *from, *to)?;
    }
    stamp(&cfg, CURRENT_FORMAT)?;

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
    let mut previous: Option<u32> = None;
    for (next, m) in (store.next_id(items)..).zip(cfg.milestones.iter()) {
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
    doc["format"] = toml_edit::value(format as i64);
    crate::store::write_atomic(&path, doc.to_string().as_bytes())
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

fn effect_of(from: u32, to: u32, cfg: &Config, _items: &[crate::item::Item]) -> Effect {
    match (from, to) {
        (1, 2) => Effect {
            rewritten: vec![CONFIG_FILE.to_string()],
            created: cfg.milestones.len(),
            modified: Vec::new(),
            summary: "milestones move from cairn.toml into items".into(),
        },
        _ => Effect {
            summary: "unknown step".into(),
            ..Default::default()
        },
    }
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
        assert!(!out.contains("careful"), "{out}");
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

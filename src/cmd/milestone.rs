// cairn — src/cmd/milestone.rs
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
// cairn milestone — read and edit the [[milestone]] blocks in cairn.toml.
//
// Writes go through toml_edit so hand-written comments and formatting survive.
use crate::cmd::{count_done, progress_bar};
use crate::config::{CONFIG_FILE, Config};
use crate::item::Item;
use crate::lock::Lock;
use crate::store::{Store, today};
use crate::style;
use anyhow::{Result, bail};
use clap::Subcommand;
use toml_edit::{ArrayOfTables, DocumentMut, Item as TomlItem, Table, value};

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Cmd>,
}

#[derive(Subcommand)]
pub enum Cmd {
    /// List milestones with progress
    List,
    /// Add a milestone
    Add {
        /// Short name, used in item frontmatter
        #[arg(value_name = "NAME")]
        name: String,
        /// Human-readable title
        #[arg(long, value_name = "TITLE")]
        title: Option<String>,
        /// Target date, YYYY-MM-DD
        #[arg(long, value_name = "DATE")]
        due: Option<String>,
        /// One-line description
        #[arg(long, value_name = "TEXT")]
        description: Option<String>,
    },
    /// Change a milestone's fields
    Set {
        #[arg(value_name = "NAME")]
        name: String,
        #[arg(long, value_name = "TITLE")]
        title: Option<String>,
        #[arg(long, value_name = "DATE")]
        due: Option<String>,
        #[arg(long, value_name = "TEXT")]
        description: Option<String>,
        /// Free-form state, e.g. "shipped"
        #[arg(long, value_name = "STATE")]
        status: Option<String>,
    },
    /// Remove a milestone (fails while items still reference it)
    #[command(visible_alias = "rm")]
    Remove {
        #[arg(value_name = "NAME")]
        name: String,
        /// Remove even if items still reference it
        #[arg(short, long)]
        force: bool,
    },
}

pub fn run(args: Args) -> Result<i32> {
    match args.command.unwrap_or(Cmd::List) {
        Cmd::List => list(),
        Cmd::Add {
            name,
            title,
            due,
            description,
        } => add(&name, title, due, description),
        Cmd::Set {
            name,
            title,
            due,
            description,
            status,
        } => set(&name, title, due, description, status),
        Cmd::Remove { name, force } => remove(&name, force),
    }
}

fn list() -> Result<i32> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let items = store.load_all()?;
    if cfg.milestones.is_empty() {
        eprintln!(
            "{}",
            style::dim("no milestones defined — add one with `cairn milestone add v1.0`")
        );
        return Ok(0);
    }
    for m in cfg.milestones_ordered() {
        let members: Vec<&Item> = items
            .iter()
            .filter(|i| i.milestone() == Some(m.name.as_str()))
            .collect();
        let done = count_done(&cfg, &members);
        // Pad the plain name before styling so the bars line up.
        println!(
            "{} {}  {}/{}{}",
            style::bold(&format!("{:<12}", m.name)),
            progress_bar(done, members.len(), 16),
            done,
            members.len(),
            match &m.due {
                Some(d) => style::dim(&format!("   due {d}")),
                None => String::new(),
            }
        );
        if let Some(t) = &m.title {
            println!("{:<12} {}", "", style::dim(t));
        }
        if let Some(st) = &m.status {
            println!("{:<12} {}", "", style::dim(&format!("[{st}]")));
        }
    }
    Ok(0)
}

fn add(
    name: &str,
    title: Option<String>,
    due: Option<String>,
    description: Option<String>,
) -> Result<i32> {
    let (cfg, path, mut doc, _lock) = open_doc()?;
    if cfg.milestone(name).is_some() {
        bail!("milestone `{name}` already exists");
    }
    if let Some(d) = &due {
        check_date(d)?;
    }

    let mut table = Table::new();
    table["name"] = value(name);
    if let Some(t) = &title {
        table["title"] = value(t.as_str());
    }
    if let Some(d) = &due {
        table["due"] = value(d.as_str());
    }
    if let Some(d) = &description {
        table["description"] = value(d.as_str());
    }

    let entry = doc
        .entry("milestone")
        .or_insert(TomlItem::ArrayOfTables(ArrayOfTables::new()));
    let Some(aot) = entry.as_array_of_tables_mut() else {
        bail!("{CONFIG_FILE}: `milestone` is not a list of [[milestone]] blocks");
    };
    aot.push(table);

    crate::store::write_atomic(&path, doc.to_string().as_bytes())?;
    println!("{} milestone {}", style::green("added"), style::bold(name));
    Ok(0)
}

fn set(
    name: &str,
    title: Option<String>,
    due: Option<String>,
    description: Option<String>,
    status: Option<String>,
) -> Result<i32> {
    let (_cfg, path, mut doc, _lock) = open_doc()?;
    if let Some(d) = &due {
        check_date(d)?;
    }
    let Some(aot) = doc
        .get_mut("milestone")
        .and_then(|i| i.as_array_of_tables_mut())
    else {
        bail!("no milestones are defined in {CONFIG_FILE}");
    };
    let Some(table) = aot
        .iter_mut()
        .find(|t| t.get("name").and_then(|v| v.as_str()) == Some(name))
    else {
        bail!("unknown milestone `{name}`");
    };
    // An explicit empty string clears the field, matching `cairn set`.
    for (key, val) in [
        ("title", title),
        ("due", due),
        ("description", description),
        ("status", status),
    ] {
        match val {
            None => {}
            Some(v) if v.is_empty() => {
                table.remove(key);
            }
            Some(v) => table[key] = value(v),
        }
    }
    crate::store::write_atomic(&path, doc.to_string().as_bytes())?;
    println!(
        "{} milestone {}",
        style::green("updated"),
        style::bold(name)
    );
    Ok(0)
}

fn remove(name: &str, force: bool) -> Result<i32> {
    let (cfg, path, mut doc, _lock) = open_doc()?;
    if cfg.milestone(name).is_none() {
        bail!("unknown milestone `{name}`");
    }
    let store = Store::new(&cfg);
    let users: Vec<u32> = store
        .load_all()?
        .iter()
        .filter(|i| i.milestone() == Some(name))
        .map(|i| i.id)
        .collect();
    if !users.is_empty() && !force {
        let refs: Vec<String> = users.iter().map(|i| cfg.format_id(*i)).collect();
        bail!(
            "{} item(s) still reference `{name}`: {}\nre-file them first, or pass --force \
             to clear the milestone from them",
            users.len(),
            refs.join(", ")
        );
    }

    if let Some(aot) = doc
        .get_mut("milestone")
        .and_then(|i| i.as_array_of_tables_mut())
    {
        aot.retain(|t| t.get("name").and_then(|v| v.as_str()) != Some(name));
    }
    crate::store::write_atomic(&path, doc.to_string().as_bytes())?;

    // Same rule as removing an item: a destructive command always leaves a
    // project that still validates. Forcing the removal clears the milestone
    // from whatever referenced it rather than leaving a dangling name behind.
    for id in &users {
        let mut item = store.find(*id)?;
        item.meta.milestone = None;
        item.touch(&today());
        item.save()?;
    }
    println!("{} milestone {}", style::red("removed"), style::bold(name));
    if !users.is_empty() {
        println!(
            "{} {} item(s) had their milestone cleared",
            style::yellow("updated"),
            users.len()
        );
    }
    Ok(0)
}

/// Create several milestones at once, for import. Existing names are left
/// alone, so re-running an import does not duplicate them.
pub fn add_many(cfg: &Config, names: &[&String]) -> Result<()> {
    let path = cfg.root.join(CONFIG_FILE);
    let text = std::fs::read_to_string(&path)?;
    let mut doc: DocumentMut = text.parse()?;
    let entry = doc
        .entry("milestone")
        .or_insert(TomlItem::ArrayOfTables(ArrayOfTables::new()));
    let Some(aot) = entry.as_array_of_tables_mut() else {
        bail!("{CONFIG_FILE}: `milestone` is not a list of [[milestone]] blocks");
    };
    let mut added = Vec::new();
    for name in names {
        if cfg.milestone(name).is_some() {
            continue;
        }
        let mut table = Table::new();
        table["name"] = value(name.as_str());
        table["description"] = value("Created by cairn import.");
        aot.push(table);
        added.push(name.as_str());
    }
    if added.is_empty() {
        return Ok(());
    }
    crate::store::write_atomic(&path, doc.to_string().as_bytes())?;
    eprintln!(
        "{} milestone(s) {}",
        style::green("added"),
        added.join(", ")
    );
    Ok(())
}

/// Opens cairn.toml for editing, holding the repository lock for the caller's
/// lifetime so two writers cannot interleave changes to the schema.
fn open_doc() -> Result<(Config, std::path::PathBuf, DocumentMut, Lock)> {
    let cfg = Config::discover()?;
    let lock = Lock::acquire(&cfg)?;
    let path = cfg.root.join(CONFIG_FILE);
    let text = std::fs::read_to_string(&path)?;
    let doc: DocumentMut = text.parse()?;
    Ok((cfg, path, doc, lock))
}

fn check_date(d: &str) -> Result<()> {
    if chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_err() {
        bail!("`{d}` is not a date in YYYY-MM-DD form");
    }
    Ok(())
}

// cairn — src/cmd/show.rs
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
// cairn show / edit / remove — single-item operations.
use crate::cmd::{item_json, paint_status, paint_type};
use crate::config::Config;
use crate::filter::{Ctx, resolve};
use crate::item::{Item, parse_id};
use crate::lock::Lock;
use crate::store::{Store, today};
use crate::{hooks, style};
use anyhow::{Context, Result, bail};
use clap::ArgAction;
use std::path::Path;

#[derive(clap::Args)]
pub struct Args {
    /// Item id
    #[arg(value_name = "ID")]
    pub id: String,

    /// Output JSON
    #[arg(long, action = ArgAction::SetTrue)]
    pub json: bool,

    /// Print the raw file
    #[arg(long, action = ArgAction::SetTrue)]
    pub raw: bool,

    /// Print the file path only
    #[arg(long, action = ArgAction::SetTrue)]
    pub path: bool,
}

#[derive(clap::Args)]
pub struct EditArgs {
    /// Item id
    #[arg(value_name = "ID")]
    pub id: String,
}

#[derive(clap::Args)]
pub struct RemoveArgs {
    /// Item ids
    #[arg(value_name = "ID", required = true)]
    pub ids: Vec<String>,

    /// Do not ask for confirmation
    #[arg(short, long, action = ArgAction::SetTrue)]
    pub force: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let all = store.load_for_reading()?;
    let ctx = Ctx::new(&cfg, &all);
    let item = store.find(parse_id(&args.id)?)?;

    if args.path {
        println!("{}", item.path.display());
        return Ok(0);
    }
    if args.raw {
        print!("{}", std::fs::read_to_string(&item.path)?);
        return Ok(0);
    }
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&item_json(&cfg, &item, &store, true))?
        );
        return Ok(0);
    }

    println!(
        "{} {}",
        style::dim(&cfg.format_id(item.id)),
        style::bold(item.title())
    );
    println!();

    let mut rows: Vec<(&str, String)> = Vec::new();
    rows.push(("status", paint_status(&cfg, item.status())));
    if item.kind().is_some() {
        rows.push(("type", paint_type(&cfg, item.kind())));
    }
    if let Some(m) = item.milestone() {
        let label = match cfg.milestone(m) {
            Some(def) => match &def.due {
                Some(due) => format!("{m}  {}", style::dim(&format!("due {due}"))),
                None => m.to_string(),
            },
            None => style::red(m),
        };
        rows.push(("milestone", label));
    }
    if let Some(a) = &item.meta.assignee {
        rows.push(("assignee", a.clone()));
    }
    if !item.meta.labels.is_empty() {
        rows.push(("labels", item.meta.labels.join(", ")));
    }
    for f in &cfg.fields {
        let v = resolve(&item, &ctx, &f.name);
        if !v.is_missing() {
            rows.push((f.name.as_str(), v.display()));
        }
    }
    if !item.meta.depends_on.is_empty() {
        let deps: Vec<String> = item
            .meta
            .depends_on
            .iter()
            .map(|id| match all.iter().find(|i| i.id == *id) {
                Some(dep) => {
                    let done = cfg.category(dep.status()).is_closed();
                    let mark = if done { "x" } else { " " };
                    format!("[{mark}] {} {}", cfg.format_id(*id), dep.title())
                }
                None => style::red(&format!("{} (missing)", cfg.format_id(*id))),
            })
            .collect();
        rows.push(("depends on", deps.join("\n            ")));
    }
    if let Some(c) = &item.meta.created {
        rows.push(("created", c.clone()));
    }
    if let Some(u) = &item.meta.updated {
        rows.push(("updated", u.clone()));
    }
    rows.push(("file", style::dim(&store.rel(&item.path))));

    let width = rows.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    for (k, v) in rows {
        println!("  {}  {v}", style::dim(&format!("{k:>width$}")));
    }

    let body = item.body.trim();
    if !body.is_empty() {
        println!();
        for line in body.lines() {
            println!("  {line}");
        }
    }
    Ok(0)
}

pub fn edit(args: EditArgs) -> Result<i32> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let item = store.find(parse_id(&args.id)?)?;
    launch_editor(&item.path)?;
    // The lock is taken after the editor exits, not around it: an editing
    // session can last minutes, and blocking every other writer for that long —
    // or having the lock declared stale underneath it — would both be wrong.
    let lock = Lock::acquire(&cfg)?;
    // Re-read so a malformed hand-edit is reported immediately rather than at
    // the next command.
    let mut reloaded = crate::item::Item::load(&item.path)?;
    store.sync_path(&mut reloaded)?;
    drop(lock);
    hooks::item(&cfg, &store, hooks::Event::AfterChange, &reloaded);
    Ok(0)
}

pub fn remove(args: RemoveArgs) -> Result<i32> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let mut targets = Vec::new();
    for raw in &args.ids {
        targets.push(store.find(parse_id(raw)?)?);
    }

    // Deleting an item that others depend on would leave references pointing at
    // nothing — a project cairn's own `check` calls invalid, reached through an
    // ordinary operation. The rule is that a destructive command always leaves a
    // valid project and says what else it touched; there is no option to leave
    // the wreckage, because no one wants it.
    let doomed: Vec<u32> = targets.iter().map(|t| t.id).collect();
    let mut dependents: Vec<Item> = store
        .load_all()?
        .into_iter()
        .filter(|i| !doomed.contains(&i.id))
        .filter(|i| i.meta.depends_on.iter().any(|d| doomed.contains(d)))
        .collect();

    if !args.force {
        for t in &targets {
            println!("  {}  {}", cfg.format_id(t.id), t.title());
        }
        // Say up front what else this will touch, so the confirmation is
        // informed rather than nominal.
        if !dependents.is_empty() {
            println!(
                "  {}",
                style::yellow(&format!(
                    "{} other item(s) depend on these; their references will be dropped",
                    dependents.len()
                ))
            );
        }
        eprint!("delete {} item(s)? [y/N] ", targets.len());
        use std::io::Write;
        std::io::stderr().flush()?;
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;
        if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            eprintln!("aborted");
            return Ok(1);
        }
    }

    let lock = Lock::acquire(&cfg)?;
    for t in &targets {
        std::fs::remove_file(&t.path).with_context(|| format!("removing {}", t.path.display()))?;
        println!(
            "{} {}  {}",
            style::red("deleted"),
            style::bold(&cfg.format_id(t.id)),
            t.title()
        );
    }
    for dep in dependents.iter_mut() {
        dep.meta.depends_on.retain(|d| !doomed.contains(d));
        dep.touch(&today());
        dep.save()?;
        println!(
            "{} {}  dropped reference(s) to the removed item(s)",
            style::yellow("updated"),
            style::bold(&cfg.format_id(dep.id))
        );
    }
    drop(lock);

    for t in &targets {
        hooks::item(&cfg, &store, hooks::Event::AfterRemove, t);
    }
    for dep in &dependents {
        hooks::item(&cfg, &store, hooks::Event::AfterChange, dep);
    }
    Ok(0)
}

/// The editor to open when neither VISUAL nor EDITOR is set.
///
/// `vi` is required by POSIX, so it is a safe assumption on a Unix. It is not
/// present on Windows, where the fallback has to be something that ships with
/// the system, and `notepad` is the only such thing that has always been there.
#[cfg(windows)]
const DEFAULT_EDITOR: &str = "notepad";
#[cfg(not(windows))]
const DEFAULT_EDITOR: &str = "vi";

pub fn launch_editor(path: &Path) -> Result<()> {
    // Each variable is checked for emptiness on its own. Applying the filter
    // after the chain looks equivalent and is not: `var("VISUAL")` returns
    // `Ok("")` for an exported-but-empty variable, so `or_else` never runs and
    // an empty VISUAL silently shadows a perfectly good EDITOR.
    fn from_env(name: &str) -> Option<String> {
        std::env::var(name)
            .ok()
            .filter(|value| !value.trim().is_empty())
    }
    let chosen = from_env("VISUAL").or_else(|| from_env("EDITOR"));
    let from_environment = chosen.is_some();
    let editor = chosen.unwrap_or_else(|| DEFAULT_EDITOR.to_string());

    let status = match std::process::Command::new(&editor).arg(path).status() {
        Ok(s) => s,
        // The common failure is not a broken editor but no editor: an empty
        // environment, or a machine without the one cairn guessed. Saying which
        // variable to set is the whole of the fix, so say it rather than
        // reporting that a process could not be spawned.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let source = if from_environment {
                "VISUAL or EDITOR names"
            } else {
                "cairn fell back to"
            };
            bail!(
                "no editor: {source} `{editor}`, which is not on PATH\n\
                 set EDITOR to one that is, or edit {} directly",
                path.display()
            );
        }
        Err(e) => return Err(e).with_context(|| format!("launching editor `{editor}`")),
    };

    if !status.success() {
        bail!("editor `{editor}` exited with {status}");
    }
    Ok(())
}

// cairn — src/cmd/show.rs
//
// Copyright (c) 2026 Oddur Sigurdsson. MIT licensed; see LICENSE.
// cairn show / edit / remove — single-item operations.
use crate::cmd::{item_json, paint_status, paint_type};
use crate::config::Config;
use crate::filter::{Ctx, resolve};
use crate::item::Item;
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

    /// Print the acceptance criteria, numbered as `cairn tick` numbers them
    #[arg(long, action = ArgAction::SetTrue)]
    pub criteria: bool,
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
    let item = store.find_ref(&args.id)?;

    if args.path {
        println!("{}", item.path.display());
        return Ok(0);
    }
    if args.raw {
        print!("{}", std::fs::read_to_string(&item.path)?);
        return Ok(0);
    }
    // `--criteria` selects what to print and `--json` how, which keeps the
    // criteria out of the item document: that shape is the interchange format's
    // and is pinned by the golden corpus, and a derived count beside the body it
    // is derived from would give a round trip two places to disagree.
    if args.criteria {
        let list = item.criteria_list(cfg.project.criteria_section.as_deref());
        if args.json {
            println!("{}", serde_json::to_string_pretty(&criteria_json(&list))?);
            return Ok(0);
        }
        if list.is_empty() {
            eprintln!("{}", style::dim("no acceptance criteria"));
            return Ok(0);
        }
        crate::cmd::tick::print_criteria(&list);
        let done = list.iter().filter(|c| c.ticked).count();
        println!();
        println!(
            "{}",
            style::dim(&format!("{done} of {} ticked", list.len()))
        );
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
        let label = match ctx.milestones.get(m) {
            Some(ms) => match crate::refs::due(ms) {
                Some(due) => format!("{m}  {}", style::dim(&format!("due {due}"))),
                None => m.to_string(),
            },
            // A milestone nothing answers to: `check` reports it, and the
            // colour is so it is visible here too.
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
    let criteria = item.criteria(cfg.project.criteria_section.as_deref());
    if criteria.any() {
        // `cairn show 12 --criteria` prints them in full; the summary here is
        // what tells somebody there is something to print.
        let text = criteria.display();
        rows.push((
            "criteria",
            if criteria.complete() {
                style::green(&text)
            } else {
                text
            },
        ));
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
    let item = store.find_ref(&args.id)?;
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
        targets.push(store.find_ref(raw)?);
    }

    // Deleting an item that others depend on would leave references pointing at
    // nothing — a project cairn's own `check` calls invalid, reached through an
    // ordinary operation. The rule is that a destructive command always leaves a
    // valid project and says what else it touched; there is no option to leave
    // the wreckage, because no one wants it.
    let doomed: Vec<u32> = targets.iter().map(|t| t.id).collect();
    let all = store.load_all()?;
    // Everything that names one of these, through `depends_on` or through any
    // declared reference. Only `depends_on` was repaired before, which was
    // right when it was the only relationship there was — and left a milestone
    // named by twenty items dangling the moment milestones became items.
    let refs = cfg.all_ref_fields();
    let names_doomed = |i: &Item| {
        i.meta.depends_on.iter().any(|d| doomed.contains(d))
            || refs.iter().any(|def| {
                crate::refs::values(i, def)
                    .iter()
                    .filter_map(|v| crate::refs::resolve(&all, def, v))
                    .any(|found| doomed.contains(&found.id))
            })
    };
    let mut dependents: Vec<Item> = all
        .iter()
        .filter(|i| !doomed.contains(&i.id))
        .filter(|i| names_doomed(i))
        .cloned()
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
        for def in &refs {
            let kept: Vec<String> = crate::refs::values(dep, def)
                .into_iter()
                .filter(|v| {
                    crate::refs::resolve(&all, def, v)
                        .is_none_or(|found| !doomed.contains(&found.id))
                })
                .collect();
            if kept.len() != crate::refs::values(dep, def).len() {
                dep.set_extra(
                    &def.name,
                    match (kept.is_empty(), crate::refs::is_many(def)) {
                        (true, _) => None,
                        (false, true) => Some(crate::item::Field::List(kept)),
                        (false, false) => {
                            Some(crate::item::Field::Text(kept.into_iter().next().unwrap()))
                        }
                    },
                );
            }
        }
        // `milestone` is typed on the item rather than kept among the custom
        // fields, so clearing it is separate — the same arrangement
        // `depends_on` has.
        if let Some(m) = dep.meta.milestone.clone()
            && let Some(def) = refs.iter().find(|d| d.name == "milestone")
            && crate::refs::resolve(&all, def, &m).is_some_and(|f| doomed.contains(&f.id))
        {
            dep.meta.milestone = None;
        }
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

/// The criteria as `cairn show --json` carries them: the same numbering `tick`
/// takes, so a script can read the list and act on it without parsing Markdown
/// itself — which is the thing this whole feature exists to stop people doing.
fn criteria_json(list: &[crate::item::Criterion]) -> serde_json::Value {
    use serde_json::json;
    json!({
        "done": list.iter().filter(|c| c.ticked).count(),
        "total": list.len(),
        "items": list
            .iter()
            .enumerate()
            .map(|(n, c)| json!({ "n": n + 1, "ticked": c.ticked, "text": c.text }))
            .collect::<Vec<_>>(),
    })
}

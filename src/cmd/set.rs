// cairn — src/cmd/set.rs
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
// cairn set / close / reopen — mutating item fields, with schema validation.
use crate::config::{Config, FieldKind};
use crate::item::{Field, Item, split_list};
use crate::lock::Lock;
use crate::store::{Store, today};
use crate::{Assign, hooks, parse_assignment, style};
use anyhow::{Result, bail};
use clap::ArgAction;

#[derive(clap::Args)]
pub struct Args {
    /// Item ids, then assignments: `cairn set 1 2 3 status=doing`
    ///
    /// Ids and assignments share one list because an assignment always
    /// contains `=` and an id never can, so the split is unambiguous and the
    /// command reads the way the other bulk commands do.
    #[arg(value_name = "ID... FIELD=VALUE...", required = true)]
    pub args: Vec<String>,

    /// Change every item matching a filter instead of naming ids
    #[arg(long, value_name = "EXPR")]
    pub filter: Option<String>,

    /// Do not ask before a filtered change
    #[arg(short = 'y', long, action = ArgAction::SetTrue)]
    pub yes: bool,

    /// Print nothing on success
    #[arg(short, long, action = ArgAction::SetTrue)]
    pub quiet: bool,
}

#[derive(clap::Args)]
pub struct CloseArgs {
    /// Item ids
    #[arg(value_name = "ID", required = true)]
    pub ids: Vec<String>,

    /// Status to move to (defaults to the first `done` status)
    #[arg(short, long, value_name = "STATUS")]
    pub status: Option<String>,

    #[arg(short, long, action = ArgAction::SetTrue)]
    pub quiet: bool,
}

#[derive(clap::Args)]
pub struct ReopenArgs {
    /// Item ids
    #[arg(value_name = "ID", required = true)]
    pub ids: Vec<String>,

    /// Status to move to (defaults to the project's default status)
    #[arg(short, long, value_name = "STATUS")]
    pub status: Option<String>,

    #[arg(short, long, action = ArgAction::SetTrue)]
    pub quiet: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);

    let (ids, assignments) = split_arguments(&args)?;
    if assignments.is_empty() {
        bail!("nothing to set: give at least one `field=value`");
    }
    // Parsed before anything is written, so a typo in the third assignment does
    // not leave the first two applied to half the items.
    let parsed: Vec<(String, Assign)> = assignments
        .iter()
        .map(|raw| parse_assignment(raw))
        .collect::<Result<_>>()?;

    let targets: Vec<u32> = match &args.filter {
        None => ids
            .iter()
            .map(|raw| cfg.parse_id(raw))
            .collect::<Result<_>>()?,
        Some(expr) => {
            let items = store.load_all()?;
            let ctx = crate::filter::Ctx::new(&cfg, &items);
            let filter = crate::filter::Filter::parse(expr)?;
            let matched: Vec<&Item> = items.iter().filter(|i| filter.matches(i, &ctx)).collect();
            if matched.is_empty() {
                bail!("no item matches `{expr}`");
            }
            if !args.yes && !confirm(&cfg, &matched)? {
                eprintln!("aborted");
                return Ok(1);
            }
            matched.iter().map(|i| i.id).collect()
        }
    };

    // One lock for the whole operation: a bulk edit must not be a window during
    // which the backlog is half-changed.
    let lock = Lock::acquire(&cfg)?;
    let mut changed = Vec::new();
    for id in &targets {
        let mut item = store.find(*id)?;
        let before = item.meta.depends_on.clone();
        let before_key = item.meta.key.clone();
        for (key, assign) in &parsed {
            // A failure part-way through has already written the items before
            // it, so the error says which — silence here would leave somebody
            // guessing how far it got.
            apply_requested(&mut item, &cfg, key, assign.clone()).map_err(|e| {
                if changed.is_empty() {
                    e
                } else {
                    e.context(format!(
                        "already written: {}",
                        changed
                            .iter()
                            .map(|i: &Item| cfg.format_id(i.id))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                }
            })?;
        }
        if item.meta.depends_on != before {
            check_no_cycle(&store, &item)?;
        }
        crate::refs::validate_on_write(&cfg, &store, &item)?;
        // A key is what other items call this one, so changing it is a rename
        // rather than an assignment. Left alone, every reference would be
        // orphaned silently; `renumber` already rewrites references when an
        // identifier moves, and this is that, one level up.
        let renamed = match (&before_key, &item.meta.key) {
            (Some(old), Some(new)) if old != new => {
                crate::refs::rename_key(&cfg, &store, &item, old, new)?
            }
            _ => Vec::new(),
        };
        item.touch(&today());
        item.save()?;
        store.sync_path(&mut item)?;
        if !args.quiet {
            println!(
                "{} {}  {}",
                style::green("updated"),
                style::bold(&cfg.format_id(item.id)),
                item.title()
            );
            for id in &renamed {
                println!("  {} {}", style::dim("also"), cfg.format_id(*id));
            }
        }
        changed.push(item);
    }
    drop(lock);

    for item in &changed {
        hooks::item(&cfg, &store, hooks::Event::AfterChange, item);
    }
    Ok(0)
}

/// Split the positional list into ids and assignments.
///
/// An assignment contains `=`; an id cannot. Once the first assignment is seen
/// everything after it must be one, so `cairn set 1 status=doing 2` is refused
/// rather than quietly ignoring the trailing id.
fn split_arguments(args: &Args) -> Result<(Vec<String>, Vec<String>)> {
    let mut ids = Vec::new();
    let mut assignments = Vec::new();
    for arg in &args.args {
        if arg.contains('=') {
            assignments.push(arg.clone());
        } else if assignments.is_empty() {
            ids.push(arg.clone());
        } else {
            bail!(
                "`{arg}` looks like an id but comes after an assignment\n\
                 ids go first: cairn set {} {}",
                ids.iter()
                    .chain(std::iter::once(arg))
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" "),
                assignments.join(" ")
            );
        }
    }
    if args.filter.is_some() && !ids.is_empty() {
        bail!("give ids or --filter, not both");
    }
    if args.filter.is_none() && ids.is_empty() {
        bail!("no item named: give an id, or --filter to select by field");
    }
    Ok((ids, assignments))
}

/// A filtered write is the dangerous one: the person running it has not seen
/// the list. Showing it is the whole point, so the confirmation is informed
/// rather than nominal.
fn confirm(cfg: &Config, matched: &[&Item]) -> Result<bool> {
    for item in matched {
        println!("  {}  {}", cfg.format_id(item.id), item.title());
    }
    eprint!("change {} item(s)? [y/N] ", matched.len());
    use std::io::Write;
    std::io::stderr().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

pub fn close(args: CloseArgs) -> Result<i32> {
    let cfg = Config::discover()?;
    let target = match &args.status {
        Some(s) => s.clone(),
        None => match cfg.done_status() {
            Some(s) => s.name.clone(),
            None => bail!(
                "no status with category = \"done\" is defined in cairn.toml; \
                 pass --status explicitly"
            ),
        },
    };
    transition(&cfg, &args.ids, &target, args.quiet, "closed")
}

pub fn reopen(args: ReopenArgs) -> Result<i32> {
    let cfg = Config::discover()?;
    let target = args
        .status
        .clone()
        .unwrap_or_else(|| cfg.initial_status().to_string());
    transition(&cfg, &args.ids, &target, args.quiet, "reopened")
}

fn transition(cfg: &Config, ids: &[String], status: &str, quiet: bool, verb: &str) -> Result<i32> {
    let store = Store::new(cfg);
    let lock = Lock::acquire(cfg)?;

    // Every write happens under the lock; the hooks for all of them run after
    // it is released, so a hook that calls cairn cannot deadlock against us.
    let mut changed = Vec::new();
    for raw in ids {
        let mut item = store.find_ref(raw)?;
        apply_requested(&mut item, cfg, "status", Assign::Set(status.to_string()))?;
        // Finishing ends the claim as surely as handing it back does. Left
        // behind, `claimed` would age until a closed item read as abandoned.
        if cfg.category(status).is_closed() {
            apply(&mut item, cfg, "claimed", Assign::Set(String::new()))?;
        }
        item.touch(&today());
        item.save()?;
        if !quiet {
            println!(
                "{} {}  {}",
                style::green(verb),
                style::bold(&cfg.format_id(item.id)),
                item.title()
            );
            // Said at the moment somebody declares the work done, against what
            // they themselves wrote down that done would mean. Never a refusal:
            // a criterion can stop applying, and a tool that blocked here would
            // teach people to tick boxes rather than to say what is true.
            if cfg.category(item.status()).is_closed() {
                let c = item.criteria(cfg.project.criteria_section.as_deref());
                if c.any() && !c.complete() {
                    eprintln!(
                        "  {}",
                        style::yellow(&format!(
                            "{} of {} acceptance criteria are unticked",
                            c.total - c.done,
                            c.total
                        ))
                    );
                }
            }
        }
        changed.push(item);
    }
    drop(lock);

    for item in &changed {
        hooks::item(cfg, &store, hooks::Event::AfterChange, item);
    }
    Ok(0)
}

/// Refuse a dependency that would close a cycle.
///
/// `cairn check` reports cycles, but an ordinary command should not be able to
/// create one: a project must not be left in a state the tool itself rejects.
pub fn check_no_cycle(store: &Store, item: &Item) -> Result<()> {
    let mut items = store.load_all()?;
    // Consider the graph as it would be once this change lands.
    if let Some(existing) = items.iter_mut().find(|i| i.id == item.id) {
        existing.meta.depends_on = item.meta.depends_on.clone();
    }
    for dep in &item.meta.depends_on {
        if let Some(path) = crate::store::dependency_path(&items, *dep, item.id) {
            // The item, then the path back to it: `0001 -> 0003 -> 0002 -> 0001`.
            // A self-dependency reads `0001 -> 0001`, which is also the truth.
            let shown: Vec<String> = std::iter::once(item.id)
                .chain(path)
                .map(|id| store.cfg.format_id(id))
                .collect();
            bail!(
                "that dependency would create a cycle: {}\nnothing could ever be started",
                shown.join(" -> ")
            );
        }
    }
    Ok(())
}

/// Apply one assignment, validating against the schema first. This is the only
/// write path for field values, so `new` and `set` cannot drift apart.
/// Apply an assignment somebody asked for, subject to what an agent may do.
///
/// Separate from `apply` because `apply` also serves schema defaults on a new
/// item and every field an import carries. Checking there refused an agent
/// permission to *create* anything in a project with a read-only field, since
/// the default was applied through the same path — the restriction is about
/// what somebody changes, not about what a schema fills in.
pub fn apply_requested(item: &mut Item, cfg: &Config, key: &str, assign: Assign) -> Result<()> {
    crate::refs::permitted_for_agent(
        cfg,
        key,
        match &assign {
            Assign::Set(v) => Some(v.as_str()),
            _ => None,
        },
    )?;
    apply(item, cfg, key, assign)
}

pub fn apply(item: &mut Item, cfg: &Config, key: &str, assign: Assign) -> Result<()> {
    match key {
        "id" => bail!("`id` cannot be changed"),
        "key" => match assign {
            Assign::Set(v) if v.trim().is_empty() => item.meta.key = None,
            Assign::Set(v) => {
                let v = v.trim().to_string();
                // The same rule identifier prefixes obey: a key that reads as a
                // number would make a reference ambiguous, and two spellings
                // must not be able to name different items.
                if cfg.id_format().read(&v).is_ok() {
                    bail!(
                        "key `{v}` reads as an identifier; a reference to it \
                         could not be told from a number"
                    );
                }
                item.meta.key = Some(v);
            }
            _ => bail!("`key` is not a list field; use key=..."),
        },
        "claimed" => match assign {
            Assign::Set(v) if v.trim().is_empty() => item.meta.claimed = None,
            Assign::Set(v) => item.meta.claimed = Some(v),
            _ => bail!("`claimed` is not a list field; use claimed=..."),
        },
        "owner" => match assign {
            Assign::Set(v) if v.trim().is_empty() => item.meta.owner = None,
            Assign::Set(v) => item.meta.owner = Some(v),
            _ => bail!("`owner` is not a list field; use owner=..."),
        },
        "created_by" => match assign {
            Assign::Set(v) if v.trim().is_empty() => item.meta.created_by = None,
            Assign::Set(v) => item.meta.created_by = Some(v),
            _ => bail!("`created_by` is not a list field; use created_by=..."),
        },
        "title" => match assign {
            Assign::Set(v) if v.is_empty() => bail!("title cannot be empty"),
            Assign::Set(v) => item.meta.title = Some(v),
            _ => bail!("`title` is not a list field; use title=..."),
        },
        "type" | "kind" => match assign {
            Assign::Set(v) if v.is_empty() => item.meta.kind = None,
            Assign::Set(v) => {
                if cfg.item_type(&v).is_none() {
                    bail!(
                        "{}",
                        unknown("type", &v, cfg.types.iter().map(|t| t.name.as_str()))
                    );
                }
                item.meta.kind = Some(v);
            }
            _ => bail!("`type` is not a list field; use type=..."),
        },
        "status" => match assign {
            Assign::Set(v) => {
                if cfg.status(&v).is_none() {
                    bail!(
                        "{}",
                        unknown("status", &v, cfg.statuses.iter().map(|s| s.name.as_str()))
                    );
                }
                item.meta.status = Some(v);
            }
            _ => bail!("`status` is not a list field; use status=..."),
        },
        // Validated by the generic ref check rather than here: whether a
        // milestone exists is a question about the backlog, and this function
        // sees only the schema. `depends_on` has the same arrangement.
        "milestone" => match assign {
            Assign::Set(v) if v.trim().is_empty() => item.meta.milestone = None,
            Assign::Set(v) => item.meta.milestone = Some(v.trim().to_string()),
            _ => bail!("`milestone` is not a list field; use milestone=..."),
        },
        "assignee" => match assign {
            Assign::Set(v) if v.is_empty() => item.meta.assignee = None,
            Assign::Set(v) => item.meta.assignee = Some(v),
            _ => bail!("`assignee` is not a list field; use assignee=..."),
        },
        "created" | "updated" => match assign {
            Assign::Set(v) => {
                check_date(key, &v)?;
                if key == "created" {
                    item.meta.created = Some(v);
                } else {
                    item.meta.updated = Some(v);
                }
            }
            _ => bail!("`{key}` is not a list field"),
        },
        "labels" | "label" => {
            let list = &mut item.meta.labels;
            match assign {
                Assign::Set(v) => *list = split_list(&v),
                Assign::Add(v) => {
                    for l in split_list(&v) {
                        if !list.iter().any(|x| x.eq_ignore_ascii_case(&l)) {
                            list.push(l);
                        }
                    }
                }
                Assign::Remove(v) => {
                    let drop = split_list(&v);
                    list.retain(|x| !drop.iter().any(|d| d.eq_ignore_ascii_case(x)));
                }
            }
        }
        "depends_on" => {
            // Through the project's own format, so `depends_on+=MP-1002` works
            // wherever `cairn show MP-1002` does.
            let parse = |v: &str| -> Result<Vec<u32>> {
                split_list(v).iter().map(|s| cfg.parse_id(s)).collect()
            };
            let list = &mut item.meta.depends_on;
            match assign {
                Assign::Set(v) => *list = parse(&v)?,
                Assign::Add(v) => {
                    for id in parse(&v)? {
                        if id == item.id {
                            bail!("an item cannot depend on itself");
                        }
                        if !list.contains(&id) {
                            list.push(id);
                        }
                    }
                }
                Assign::Remove(v) => {
                    let drop = parse(&v)?;
                    list.retain(|x| !drop.contains(x));
                }
            }
            item.meta.depends_on.sort_unstable();
        }
        other => apply_custom(item, cfg, other, assign)?,
    }
    Ok(())
}

fn apply_custom(item: &mut Item, cfg: &Config, key: &str, assign: Assign) -> Result<()> {
    let Some(def) = cfg.field(key) else {
        let known: Vec<&str> = crate::config::RESERVED_FIELDS
            .iter()
            .copied()
            .chain(cfg.fields.iter().map(|f| f.name.as_str()))
            .collect();
        bail!(
            "unknown field `{key}`\nknown fields: {}\ndefine it with a [[field]] block in cairn.toml",
            known.join(", ")
        );
    };

    match def.kind {
        // Checked here rather than in `validate_scalar`, which sees only the
        // definition: whether a name resolves is a question about the backlog.
        FieldKind::Ref => {
            let mut current = match item.get(key) {
                Field::List(v) => v,
                Field::Text(t) if !t.is_empty() => vec![t],
                _ => vec![],
            };
            match assign {
                Assign::Set(v) => current = split_list(&v),
                Assign::Add(v) => {
                    for x in split_list(&v) {
                        if !current.iter().any(|c| c.eq_ignore_ascii_case(&x)) {
                            current.push(x);
                        }
                    }
                }
                Assign::Remove(v) => {
                    let drop = split_list(&v);
                    current.retain(|x| !drop.iter().any(|d| d.eq_ignore_ascii_case(x)));
                }
            }
            if !crate::refs::is_many(def) && current.len() > 1 {
                bail!("`{key}` names one item, but {} were given", current.len());
            }
            // An id-addressed ref stores numbers, so that it reads the way
            // `depends_on` does and a hand-written `part_of: [1, 4]` survives a
            // save unchanged.
            let numeric: Option<Vec<u32>> = (def.by == crate::config::Addressing::Id)
                .then(|| {
                    current
                        .iter()
                        .map(|v| v.trim().trim_start_matches('#').parse::<u32>().ok())
                        .collect::<Option<Vec<u32>>>()
                })
                .flatten();
            match (numeric, crate::refs::is_many(def)) {
                (Some(ids), true) => item.set_extra_ids(key, &ids),
                (Some(ids), false) => item.set_extra_id(key, ids.first().copied()),
                (None, true) => {
                    item.set_extra(key, (!current.is_empty()).then_some(Field::List(current)))
                }
                (None, false) => item.set_extra(
                    key,
                    (!current.is_empty()).then(|| Field::Text(current.remove(0))),
                ),
            }
        }
        FieldKind::List => {
            let mut current = match item.get(key) {
                Field::List(v) => v,
                Field::Text(t) if !t.is_empty() => vec![t],
                _ => vec![],
            };
            match assign {
                Assign::Set(v) => current = split_list(&v),
                Assign::Add(v) => {
                    for x in split_list(&v) {
                        if !current.iter().any(|c| c.eq_ignore_ascii_case(&x)) {
                            current.push(x);
                        }
                    }
                }
                Assign::Remove(v) => {
                    let drop = split_list(&v);
                    current.retain(|c| !drop.iter().any(|d| d.eq_ignore_ascii_case(c)));
                }
            }
            item.set_extra(key, Some(Field::List(current)));
        }
        _ => {
            let Assign::Set(value) = assign else {
                bail!("`{key}` is not a list field; use {key}=value");
            };
            if value.is_empty() {
                if def.required {
                    bail!("`{key}` is required and cannot be cleared");
                }
                item.set_extra(key, None);
                return Ok(());
            }
            validate_scalar(def, &value)?;
            item.set_extra(key, Some(Field::Text(value)));
        }
    }
    Ok(())
}

pub fn validate_scalar(def: &crate::config::FieldDef, value: &str) -> Result<()> {
    match def.kind {
        FieldKind::Enum => {
            if !def.values.iter().any(|v| v.eq_ignore_ascii_case(value)) {
                // One line: this message also appears in `cairn check` output,
                // where each diagnostic must stay on its own line.
                bail!(
                    "`{}` must be one of {} (got `{value}`)",
                    def.name,
                    def.values.join(", ")
                );
            }
        }
        // A ref names something that has to exist, which cannot be checked
        // against the definition alone. `check` and the write path resolve it
        // where the items are in hand.
        FieldKind::Ref => {}
        FieldKind::Date => check_date(&def.name, value)?,
        FieldKind::Number => {
            if value.parse::<f64>().is_err() {
                bail!("`{}` must be a number, got `{value}`", def.name);
            }
        }
        FieldKind::Bool => {
            if !matches!(
                value.to_ascii_lowercase().as_str(),
                "true" | "false" | "yes" | "no"
            ) {
                bail!("`{}` must be true or false, got `{value}`", def.name);
            }
        }
        FieldKind::Text | FieldKind::List => {}
    }
    Ok(())
}

fn check_date(name: &str, value: &str) -> Result<()> {
    if chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_err() {
        bail!("`{name}` must be a date in YYYY-MM-DD form, got `{value}`");
    }
    Ok(())
}

fn unknown<'a>(kind: &str, value: &str, known: impl Iterator<Item = &'a str>) -> String {
    let list: Vec<&str> = known.collect();
    if list.is_empty() {
        format!("unknown {kind} `{value}` (none are defined in cairn.toml)")
    } else {
        format!("unknown {kind} `{value}`\nknown: {}", list.join(", "))
    }
}

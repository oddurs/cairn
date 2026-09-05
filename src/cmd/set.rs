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
use crate::item::{Field, Item, parse_id, split_list};
use crate::lock::Lock;
use crate::store::{Store, today};
use crate::{Assign, hooks, parse_assignment, style};
use anyhow::{Result, bail};
use clap::ArgAction;

#[derive(clap::Args)]
pub struct Args {
    /// Item id
    #[arg(value_name = "ID")]
    pub id: String,

    /// Assignments: `status=doing`, `labels+=auth`, `assignee=`
    #[arg(value_name = "FIELD=VALUE", required = true)]
    pub assignments: Vec<String>,

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
    let lock = Lock::acquire(&cfg)?;
    let mut item = store.find(parse_id(&args.id)?)?;

    let before = item.meta.depends_on.clone();
    for raw in &args.assignments {
        let (key, assign) = parse_assignment(raw)?;
        apply(&mut item, &cfg, &key, assign)?;
    }
    if item.meta.depends_on != before {
        check_no_cycle(&store, &item)?;
    }
    item.touch(&today());
    item.save()?;
    store.sync_path(&mut item)?;
    drop(lock);
    hooks::item(&cfg, &store, hooks::Event::AfterChange, &item);

    if !args.quiet {
        println!(
            "{} {}  {}",
            style::green("updated"),
            style::bold(&cfg.format_id(item.id)),
            item.title()
        );
    }
    Ok(0)
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
        let mut item = store.find(parse_id(raw)?)?;
        apply(&mut item, cfg, "status", Assign::Set(status.to_string()))?;
        item.touch(&today());
        item.save()?;
        if !quiet {
            println!(
                "{} {}  {}",
                style::green(verb),
                style::bold(&cfg.format_id(item.id)),
                item.title()
            );
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
pub fn apply(item: &mut Item, cfg: &Config, key: &str, assign: Assign) -> Result<()> {
    match key {
        "id" => bail!("`id` cannot be changed"),
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
        "milestone" => match assign {
            Assign::Set(v) if v.is_empty() => item.meta.milestone = None,
            Assign::Set(v) => {
                if cfg.milestone(&v).is_none() {
                    bail!(
                        "{}\nadd it with: cairn milestone add {v}",
                        unknown(
                            "milestone",
                            &v,
                            cfg.milestones.iter().map(|m| m.name.as_str())
                        )
                    );
                }
                item.meta.milestone = Some(v);
            }
            _ => bail!("`milestone` is not a list field; use milestone=..."),
        },
        "source" => match assign {
            Assign::Set(v) if v.is_empty() => item.meta.source = None,
            Assign::Set(v) => item.meta.source = Some(v),
            _ => bail!("`source` is not a list field; use source=..."),
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
            let parse = |v: &str| -> Result<Vec<u32>> {
                split_list(v).iter().map(|s| parse_id(s)).collect()
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
